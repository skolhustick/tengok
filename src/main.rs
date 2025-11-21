use std::{
    collections::HashMap,
    env,
    fs::File,
    io::{self, BufRead, BufReader},
    path::{Path, PathBuf},
    process,
    sync::Arc,
};

use crossbeam_channel::unbounded;
use humansize::{DECIMAL, format_size};
use ignore::{WalkBuilder, WalkState};
use owo_colors::OwoColorize;
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone)]
struct FileStat {
    path: PathBuf,
    size: u64,
    lines: u64,
}

#[derive(Debug, Default)]
struct Summary {
    total_files: u64,
    total_size: u64,
    total_lines: u64,
    max_lines_file: Option<FileStat>,
    largest_dir: Option<(PathBuf, u64)>, // (path, size)
}

const DEFAULT_MAX_LINE_BYTES: u64 = 5 * 1024 * 1024; // ~5MB
const BINARY_EXTS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "webp", "ico", "svg", "tif", "tiff", "pdf", "zip", "gz",
    "bz2", "xz", "7z", "tar", "rar", "mp4", "mov", "avi", "mkv", "mp3", "wav", "flac", "ogg",
    "ttf", "otf", "woff", "woff2", "exe", "dll", "so", "dylib", "class", "jar", "bin",
];

struct Config {
    root: PathBuf,
    plain: bool,
    skip_lines: bool,
    force_lines: bool,
    max_line_bytes: u64,
}

impl Config {
    fn from_args() -> Result<Self, String> {
        let mut args = env::args().skip(1).peekable();
        let mut root: Option<PathBuf> = None;
        let mut plain = false;
        let mut skip_lines = false;
        let mut force_lines = false;
        let mut max_line_bytes = DEFAULT_MAX_LINE_BYTES;

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--plain" | "--no-colors" => plain = true,
                "--no-lines" => skip_lines = true,
                "--force-lines" => {
                    force_lines = true;
                    skip_lines = false;
                }
                "--max-line-bytes" => {
                    let value = args
                        .next()
                        .ok_or_else(|| "--max-line-bytes requires a numeric value".to_string())?;
                    max_line_bytes = value
                        .parse()
                        .map_err(|_| "Unable to parse --max-line-bytes".to_string())?;
                }
                _ if arg.starts_with("--max-line-bytes=") => {
                    let value = arg.split_once('=').unwrap().1;
                    max_line_bytes = value
                        .parse()
                        .map_err(|_| "Unable to parse --max-line-bytes".to_string())?;
                }
                _ if arg.starts_with('-') => {
                    return Err(format!("Unknown flag: {}", arg));
                }
                _ => {
                    root = Some(PathBuf::from(arg));
                }
            }
        }

        let root = root.unwrap_or_else(|| PathBuf::from("."));
        Ok(Self {
            root,
            plain,
            skip_lines,
            force_lines,
            max_line_bytes,
        })
    }
}

fn usage() -> &'static str {
    "Usage: tengok [OPTIONS] [PATH]

Options:
  --plain, --no-colors        Disable ANSI colors in the report
  --no-lines                  Skip line counting entirely
  --force-lines               Always count lines (even for large/binary files)
  --max-line-bytes <N>        Only count lines for files up to N bytes (default ~5MB)
"
}

#[derive(Debug)]
struct FileRecord {
    path: PathBuf,
    parent: PathBuf,
    size: u64,
    lines: u64,
}

fn main() -> io::Result<()> {
    let config = match Config::from_args() {
        Ok(cfg) => Arc::new(cfg),
        Err(err) => {
            eprintln!("{}", err);
            eprintln!("{}", usage());
            process::exit(1);
        }
    };

    if !config.root.exists() {
        eprintln!("Path does not exist: {}", config.root.display());
        process::exit(1);
    }

    let summary = scan_dir(&config)?;
    print_report(&config, &summary);

    Ok(())
}

fn scan_dir(config: &Arc<Config>) -> io::Result<Summary> {
    let root = config.root.clone();
    let (tx, rx) = unbounded::<FileRecord>();

    let walker = WalkBuilder::new(&root).git_ignore(true).build_parallel();

    let config_for_threads = Arc::clone(config);
    let root_for_threads = root.clone();

    walker.run(|| {
        let tx = tx.clone();
        let config = Arc::clone(&config_for_threads);
        let root = root_for_threads.clone();
        let mut line_buf = Vec::with_capacity(64 * 1024);
        Box::new(move |result| {
            let dent = match result {
                Ok(d) => d,
                Err(_) => return WalkState::Continue,
            };

            if !dent.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                return WalkState::Continue;
            }

            let path = dent.into_path();
            let meta = match path.metadata() {
                Ok(m) => m,
                Err(_) => return WalkState::Continue,
            };

            let size = meta.len();
            let lines = if should_count_lines(&path, size, &config) {
                count_lines_fast(&path, &mut line_buf).unwrap_or(0)
            } else {
                0
            };

            let parent = path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| root.clone());

            if tx
                .send(FileRecord {
                    path,
                    parent,
                    size,
                    lines,
                })
                .is_err()
            {
                return WalkState::Quit;
            }

            WalkState::Continue
        })
    });

    drop(tx);

    let mut summary = Summary::default();
    let mut dir_sizes: HashMap<PathBuf, u64> = HashMap::new();

    for record in rx {
        summary.total_files += 1;
        summary.total_size += record.size;
        summary.total_lines += record.lines;

        let current_max = summary
            .max_lines_file
            .as_ref()
            .map(|f| f.lines)
            .unwrap_or(0);

        if record.lines > current_max {
            summary.max_lines_file = Some(FileStat {
                path: record.path.clone(),
                size: record.size,
                lines: record.lines,
            });
        }

        *dir_sizes.entry(record.parent).or_insert(0) += record.size;
    }

    if let Some((dir, size)) = dir_sizes.into_iter().max_by_key(|(_, s)| *s) {
        summary.largest_dir = Some((dir, size));
    }

    Ok(summary)
}

fn count_lines_fast(path: &Path, buf: &mut Vec<u8>) -> io::Result<u64> {
    let file = File::open(path)?;
    let mut reader = BufReader::with_capacity(64 * 1024, file);
    let mut lines: u64 = 0;

    loop {
        buf.clear();
        let bytes = reader.read_until(b'\n', buf)?;
        if bytes == 0 {
            break;
        }
        lines += 1;
    }

    Ok(lines)
}

fn should_count_lines(path: &Path, size: u64, config: &Config) -> bool {
    if config.skip_lines {
        return false;
    }
    if config.force_lines {
        return true;
    }
    if config.max_line_bytes > 0 && size > config.max_line_bytes {
        return false;
    }
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        let ext_lower = ext.to_ascii_lowercase();
        if BINARY_EXTS.contains(&ext_lower.as_str()) {
            return false;
        }
    }
    true
}

fn print_report(config: &Config, summary: &Summary) {
    let title = format!("Folder Summary: {}", config.root.display());
    let size_human = format_size(summary.total_size, DECIMAL);

    let (largest_dir_str, largest_dir_size) = match &summary.largest_dir {
        Some((path, size)) => (path.display().to_string(), format_size(*size, DECIMAL)),
        None => ("-".to_string(), "-".to_string()),
    };

    let (max_file_name, max_file_lines, max_file_size) = match &summary.max_lines_file {
        Some(f) => (
            f.path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("<unknown>")
                .to_string(),
            f.lines,
            format_size(f.size, DECIMAL),
        ),
        None => ("-".to_string(), 0, "-".to_string()),
    };

    // Inner content width (excluding outer padding spaces)
    const INNER_WIDTH: usize = 28;

    // Helper to create a formatted row with label and value
    // Label takes 14 chars, value takes 11 chars, leaving a couple of spaces for padding
    fn format_row(label: &str, value: &str) -> (String, String) {
        let label_truncated = truncate(label, 14);
        let label_fmt = format!("{:<14}", label_truncated);
        let value_fmt = format!("{:>11}", truncate(value, 11));
        (label_fmt, value_fmt)
    }

    let (files_label, files_value_fmt) = format_row("Files:", &format_num(summary.total_files));
    let (size_label, size_value_fmt) = format_row("Size:", &size_human);
    let (lines_label, lines_value_fmt) =
        format_row("Total Lines:", &format_num(summary.total_lines));
    let largest_dir_val = format!("{} ({})", largest_dir_str, largest_dir_size);
    let (largest_dir_label, largest_dir_value_fmt) = format_row("Largest Dir:", &largest_dir_val);
    let max_file_val = format!(
        "{} ({} lines, {})",
        max_file_name,
        format_num(max_file_lines),
        max_file_size
    );
    let (max_file_label, max_file_value_fmt) = format_row("Max Lines File:", &max_file_val);

    // Color helpers
    let color_border = |s: &str| -> String {
        if config.plain {
            s.to_string()
        } else {
            format!("{}", s.bright_green())
        }
    };
    let color_label = |s: &str| -> String {
        if config.plain {
            s.to_string()
        } else {
            format!("{}", s.bright_magenta())
        }
    };
    let color_value = |s: &str| -> String {
        if config.plain {
            s.to_string()
        } else {
            format!("{}", s.bright_green())
        }
    };

    // Box borders
    let horizontal_raw = "─".repeat(INNER_WIDTH + 2);
    let border = color_border(&horizontal_raw);
    let top_left = color_border("┌");
    let top_right = color_border("┐");
    let bottom_left = color_border("└");
    let bottom_right = color_border("┘");
    let vert_symbol = color_border("│");
    let divider = color_border("├");
    let divider_right = color_border("┤");

    // Utility to print a line with padding
    let vert_left = vert_symbol.clone();
    let vert_right = vert_symbol.clone();
    let plain_mode = config.plain;
    let print_line = move |plain: &str, colored: String| {
        let visible = UnicodeWidthStr::width(plain);
        let padding = INNER_WIDTH.saturating_sub(visible);
        let body = if plain_mode {
            plain.to_string()
        } else {
            colored
        };
        println!(
            "{} {}{} {}",
            vert_left,
            body,
            " ".repeat(padding),
            vert_right
        );
    };

    println!("{}{}{}", top_left, border, top_right);

    let title_plain = truncate(&title, INNER_WIDTH);
    let title_colored = color_value(&title_plain);
    print_line(&title_plain, title_colored);

    println!("{}{}{}", divider, border, divider_right);

    let row_plain_and_colored = |label: &str, value: &str| {
        let plain = format!("{}   {}", label, value);
        let colored = format!("{}   {}", color_label(label), color_value(value));
        print_line(&plain, colored);
    };

    row_plain_and_colored(&files_label, &files_value_fmt);
    row_plain_and_colored(&size_label, &size_value_fmt);
    row_plain_and_colored(&lines_label, &lines_value_fmt);
    row_plain_and_colored(&largest_dir_label, &largest_dir_value_fmt);
    row_plain_and_colored(&max_file_label, &max_file_value_fmt);

    println!("{}{}{}", bottom_left, border, bottom_right);
}

fn format_num(n: u64) -> String {
    // basic thousand separator
    let s = n.to_string();
    let mut out = String::new();
    let bytes = s.as_bytes();
    let len = bytes.len();
    for (i, ch) in bytes.iter().enumerate() {
        out.push(*ch as char);
        let left = len - i - 1;
        if left > 0 && left % 3 == 0 {
            out.push(',');
        }
    }
    out
}

// Truncate & add "…" if too long to fit in n chars
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i >= max - 1 {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    out
}
