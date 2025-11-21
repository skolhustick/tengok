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

### 1. cargo install (Rust users)

If you already have Rust installed, the quickest path is via [crates.io](https://crates.io/crates/tengok):

```bash
cargo install tengok
```

### 2. One-line installer (recommended for binaries)

We publish signed binaries for macOS (arm64/x86_64) and Linux (arm64/x86_64) on [GitHub Releases][releases]. Running the installer with **no flags** walks you through an interactive prompt so you can decide whether to install locally or system-wide before anything is written.

```bash
curl -fsSL https://raw.githubusercontent.com/skolhustick/tengok/main/install.sh | bash
```

During the prompt you’ll pick where the binary should live:

| Mode | Description | Flag |
| --- | --- | --- |
| Local | Installs to `~/.local/bin` (no sudo) | `--local` |
| Global | Installs to `/usr/local/bin` (sudo move) | `--global` |

Additional options:

| Option | Description |
| --- | --- |
| `--force` | Overwrite an existing binary without prompting. |
| `TENGOK_VERSION=v0.1.1` | Pin a specific release (default = latest). |

Examples:

```bash
# Non-interactive local install (skips the prompt)
curl -fsSL https://raw.githubusercontent.com/skolhustick/tengok/main/install.sh | bash -s -- --local

# Install a specific tagged release globally, forcing overwrite
TENGOK_VERSION=v0.1.1 \
  curl -fsSL https://raw.githubusercontent.com/skolhustick/tengok/main/install.sh | bash -s -- --global --force
```

### 3. Manual download

Every release bundles four standalone binaries in `dist/`:

| Asset | Target |
| --- | --- |
| `tengok-macos-arm64` | Apple Silicon macOS |
| `tengok-macos-x86_64` | Intel macOS |
| `tengok-linux-arm64` | Linux ARM64 (musl) |
| `tengok-linux-x86_64` | Linux x86_64 (musl) |

Download the asset that matches your machine, `chmod +x`, and move it somewhere on your `PATH`:

```bash
curl -L https://github.com/skolhustick/tengok/releases/latest/download/tengok-linux-x86_64 -o tengok
chmod +x tengok
mv tengok ~/.local/bin/
```

### 4. Build from source

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

[releases]: https://github.com/skolhustick/tengok/releases

