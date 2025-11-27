use std::{
    collections::HashMap,
    env,
    fs::File,
    io::{self, BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process,
    sync::Arc,
    time::{Duration, Instant},
};

use crossbeam_channel::unbounded;
use crossterm::terminal;
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

    let spinner_frames: &[char] = &['-', '\\', '|', '/'];
    let mut spinner_idx: usize = 0;
    let mut last_draw = Instant::now();

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

        if !config.plain && last_draw.elapsed() >= Duration::from_millis(80) {
            last_draw = Instant::now();
            spinner_idx = (spinner_idx + 1) % spinner_frames.len();
            let frame = spinner_frames[spinner_idx];
            let path_str = display_relative_path(&record.path, &config.root);
            let path_short = ellipsize_middle(&path_str, 40);
            let files = format_num(summary.total_files);
            let size = format_size(summary.total_size, DECIMAL);
            let msg = format!(
                "{} Scanning… {} files, {} ({})",
                frame, files, size, path_short
            );
            let mut stderr = io::stderr();
            let _ = write!(stderr, "\r{}", msg);
            let _ = stderr.flush();
        }
    }

    if !config.plain {
        let mut stderr = io::stderr();
        let _ = writeln!(stderr);
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
    let files_value = format_num(summary.total_files);
    let lines_value = format_num(summary.total_lines);
    let files_value_with_unit = format!("{} Files", files_value);
    let lines_value_with_unit = format!("{} Lines", lines_value);

    let (largest_dir_str, largest_dir_size) = match &summary.largest_dir {
        Some((path, size)) => (
            display_relative_path(path, &config.root),
            format_size(*size, DECIMAL),
        ),
        None => ("-".to_string(), "-".to_string()),
    };

    let (max_file_path_raw, max_file_lines, max_file_size) = match &summary.max_lines_file {
        Some(f) => (
            display_relative_path(&f.path, &config.root),
            f.lines,
            format_size(f.size, DECIMAL),
        ),
        None => ("-".to_string(), 0, "-".to_string()),
    };

    let largest_dir_val = if largest_dir_str == "-" {
        "-".to_string()
    } else {
        format!("{} ({})", largest_dir_str, largest_dir_size)
    };
    let max_file_val = if max_file_path_raw == "-" {
        "-".to_string()
    } else {
        format!(
            "{} ({} lines, {})",
            max_file_path_raw,
            format_num(max_file_lines),
            max_file_size
        )
    };

    // Layout: label + spacing + value widths add up to inner width.
    // The box grows to fit the widest value (typically the max-line file),
    // but never exceeds the current terminal width; long values are then
    // ellipsized in the middle to stay on a single row.
    const LABEL_WIDTH: usize = 6;
    const MIN_VALUE_WIDTH: usize = 24;
    const MAX_VALUE_WIDTH: usize = 96;

    let mut value_width = [
        files_value_with_unit.as_str(),
        size_human.as_str(),
        lines_value_with_unit.as_str(),
        largest_dir_val.as_str(),
        max_file_val.as_str(),
    ]
    .into_iter()
    .map(|s| UnicodeWidthStr::width(s))
    .max()
    .unwrap_or(0)
    .clamp(MIN_VALUE_WIDTH, MAX_VALUE_WIDTH);

    if let Ok((cols, _)) = terminal::size() {
        let cols = cols as usize;
        let max_inner = cols.saturating_sub(3); // borders + spaces
        if max_inner > LABEL_WIDTH + 3 {
            let max_value = max_inner.saturating_sub(LABEL_WIDTH + 3);
            if max_value > 0 {
                value_width = value_width.min(max_value);
            }
        }
    }

    let inner_width = LABEL_WIDTH + 3 + value_width;

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
    let horizontal_raw = "─".repeat(inner_width + 2);
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
        let padding = inner_width.saturating_sub(visible);
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

    let format_row = |label: &str, value: &str| -> (String, String) {
        let label_truncated = truncate(label, LABEL_WIDTH);
        let label_fmt = format!("{:<label_w$}", label_truncated, label_w = LABEL_WIDTH);
        let value_truncated = ellipsize_middle(value, value_width);
        let value_fmt = format!("{:>value_w$}", value_truncated, value_w = value_width);
        (label_fmt, value_fmt)
    };

    let (files_label, files_value_fmt) = format_row("[F]", &files_value_with_unit);
    let (size_label, size_value_fmt) = format_row("[B]", &size_human);
    let (lines_label, lines_value_fmt) = format_row("[L]", &lines_value_with_unit);
    let (largest_label, largest_dir_value_fmt) = format_row("[D↑]", &largest_dir_val);
    let (max_lines_label, max_file_value_fmt) = format_row("[L↑]", &max_file_val);

    println!("{}{}{}", top_left, border, top_right);

    let title_plain = truncate(&title, inner_width);
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
    row_plain_and_colored(&largest_label, &largest_dir_value_fmt);
    row_plain_and_colored(&max_lines_label, &max_file_value_fmt);

    println!("{}{}{}", bottom_left, border, bottom_right);
}

fn display_relative_path(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| {
            if p.as_os_str().is_empty() {
                ".".to_string()
            } else {
                p.display().to_string()
            }
        })
        .unwrap_or_else(|_| path.display().to_string())
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

// Insert "…" in the middle to keep both ends visible within max chars
fn ellipsize_middle(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }

    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    if len <= max {
        return s.to_string();
    }
    if max == 1 {
        return "…".to_string();
    }

    let keep = max - 1;
    let front = keep / 2;
    let back = keep - front;
    let back_start = len - back;

    let mut out = String::with_capacity(max);
    for ch in chars.iter().take(front) {
        out.push(*ch);
    }
    out.push('…');
    for ch in chars.iter().skip(back_start) {
        out.push(*ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ellipsize_leaves_short_strings_alone() {
        assert_eq!(ellipsize_middle("short.txt", 20), "short.txt");
    }

    #[test]
    fn ellipsize_compacts_middle_and_keeps_ends() {
        let original = "somefilenameisverylong.txt";
        assert_eq!(ellipsize_middle(original, 20), "somefilen…rylong.txt");
    }
}
