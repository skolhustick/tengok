# tengok

`tengok` is a fast, colorized folder summary CLI written in Rust. It walks your project tree, honors `.gitignore`, and produces a compact dashboard of key stats—perfect for getting a feel for a codebase before diving in.

```
┌──────────────────────────────┐
│ Folder Summary: ./pdf-reader │
├──────────────────────────────┤
│ Files:           7,699       │
│ Size:            4.35 GB     │
│ Total Lines:     401,687     │
│ Largest Dir:     ./src (3.1G)│
│ Max Lines File:  main.rs (…) │
└──────────────────────────────┘
```

## Features
- **Parallel walker** powered by `ignore` + `crossbeam` for snappy scans, even on giant repos.
- **Smart line counting** skips obvious binaries / large blobs (configurable), or can be forced on.
- **Colorful or plain output** (`--plain`) with Unicode-aware padding to keep borders aligned.
- **Human-friendly metrics** (files, total bytes, total lines, largest directory, max-line file).

## Installation

```bash
# Build & install locally
cargo install --path .

# Or just build a release binary
cargo build --release
```

> ℹ️ The release profile ships with `lto`, `opt-level = "s"`, and stripped symbols for a small executable.

## Usage

```bash
tengok [OPTIONS] [PATH]

# Examples
tengok .                 # scan current directory
tengok --plain /tmp/app  # disable colors for piping
tengok --no-lines        # skip line counting (fastest)
tengok --force-lines     # always count lines, even for large/binary files
tengok --max-line-bytes 1048576  # only count lines for files ≤ 1 MB
```

### Options
| Flag | Description |
| ---- | ----------- |
| `--plain`, `--no-colors` | Disable ANSI colors (great for CI logs or piping). |
| `--no-lines` | Skip line counting entirely; reports `0` for total lines/max file lines. |
| `--force-lines` | Count lines for every regular file, ignoring heuristics. |
| `--max-line-bytes <N>` | Only count lines for files up to `N` bytes (default ≈ 5 MB). |

Notes:
- Hidden files and anything ignored by `.gitignore` are skipped, so Finder/Du totals will usually be higher.
- Only regular files are counted; directories, symlinks, and devices are ignored.

## Development

```bash
cargo fmt
cargo clippy --all-targets
cargo test
```

When you’re ready to cut a release, rebuild with `cargo build --release` and ship `target/release/tengok`.

---

Questions or ideas? Open an issue or pull request once the repo hits GitHub! Happy scanning. 🚀

