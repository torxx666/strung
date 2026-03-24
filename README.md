# 🧵 Strung

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-stable-brightgreen.svg)](https://www.rust-lang.org)
[![Docker](https://img.shields.io/badge/docker-ready-blue.svg)](https://www.docker.com)

**Strung** is a high-performance, intelligent string extraction tool designed as a modern, "smart" replacement for the classic Linux `strings` utility. 

It doesn't just extract printable characters—it uses linguistic analysis, entropy scoring, and deobfuscation to find the **signal** in the **noise**.

---

## ✨ Key Features

- **🚀 Blazing Fast**: Parallel multi-core scanning using `rayon` and memory-mapped I/O (`memmap2`).
- **🧠 Intelligent Filtering**: Eliminates 98%+ of binary junk using:
    - Digram frequency analysis (English/Code patterns)
    - Vowel-consonant balance check
    - Symbol-to-alpha ratio thresholding
- **🎨 Smart Mode (`-z`)**: Visual highlighting with terminal colors:
    - **Bold Green**: High-significance findings
    - **Bold Blue**: URLs, IPv4 addresses, and Emails
    - **Magenta**: Auto-decoded Base64 payloads
    - **Cyan**: XOR-obfuscated strings
- **🛡️ Pro Arsenal**:
    - **Entropy Analysis**: Identify packed/encrypted data blobs.
    - **Base64 Auto-Decoding**: Detect and decode B64 on the fly.
    - **XOR Brute-force**: Defeat single-byte XOR obfuscation.
    - **Binary Awareness**: Selective scanning of ELF/PE/Mach-O sections (e.g., `.rodata`).
- **✅ Professional Parity**: Full support for offsets (decimal/hex), regex matching, and various encodings (UTF-16 LE/BE).

---

## 🛠 Installation

### 1. Download Binaries
Download the pre-compiled binary for your architecture from the [Releases](https://github.com/torxx666/strung/releases) page.

1.  Download the ZIP/Tarball.
2.  Extract the `strung` binary.
3.  Move it to your PATH (e.g., `/usr/local/bin`).

### 2. Using Cargo (Recommended)
```bash
cargo install --path .
```

### 3. Using Docker
```bash
docker build -t strung-app .
# Alias it for convenience
alias strung='docker run --rm -v $(pwd):/data strung-app'
```

---

## 📖 Command-Line Reference

| Short | Long | Description |
|-------|------|-------------|
| `-a` | `--all` | Skip junk filtering (behave like standard `strings`) |
| `-n <N>`| `--min-len` | Set the minimum string length (default: 4) |
| `-z` | `--smart` | **Smart Mode**: Visual highlights for the most relevant results |
| `--base64` | | **Base64**: Detect and decode B64 payloads |
| `--xor` | | **XOR**: Brute-force single-byte XOR obfuscation |
| `--entropy` | | **Entropy**: Analyze file for packed/encrypted regions |
| `-S <S>`| `--section` | Scan a specific binary section (e.g., `.rodata`, `.text`) |
| `-o <O>`| `--order` | Sort results: `l` (length), `p` (prob/score), `n` (count) |
| `-t <T>`| `--show-offset`| Show file offset: `d` (decimal), `x` (hex) |
| `-m <P>`| `--match` | Only show strings matching Regex pattern `<P>` |
| `-e <E>`| `--encoding` | Set encoding: `s` (8-bit), `l` (UTF-16LE), `b` (UTF-16BE) |
| `-C <N>`| `--context` | Show `<N>` bytes of hex context around each found string |
| `-g <T>`| `--gt` | Minimum significance threshold (0.0 to 1.0) |
| `-c <F>`| `--config` | Load a custom `strung.toml` configuration file |
| `--debug`| | Output `<string> <count> <score> [origin]` for every match |
| `--silent`| | Suppress extra logging |
| `--no-sort`| | Disable all sorting (fastest output) |
| `--no-duplicate`| | Show every occurrence of a string (disable deduplication) |
| `--generate-config`| | Output a default `strung.toml` template to stdout |

---

## 📊 Comparison: strings vs. strung

On a **127MB** MP4 file:

| Tool | Processing Time | Output Lines | Results Quality |
|------|-----------------|--------------|-----------------|
| `strings` | 0.9s | 1,557,177 | 99% Junk |
| **`strung`** | **1.1s** | **33,572** | **98% Noise Reduced** |

> Even with deep linguistic analysis, `strung` provides massive noise reduction with almost no performance penalty.
> See the full [Benchmark Report](benchmark_report.md) for details.

---

## 📄 Documentation

- [Architecture Overview](ARCHITECTURE.md) - How the engine works.
- [Contributing](CONTRIBUTING.md) - How to help improve Strung.
- [Benchmark Report](benchmark_report.md) - Detailed performance metrics.

## 🗺️ Roadmap

We're just getting started! Here's what we have planned:
- [ ] **Recursive Container Scanning**: Automatically look inside `.zip`, `.tar`, and `.7z`.
- [ ] **YARA Integration**: Run standard YARA rules against extracted strings.
- [ ] **Multi-Language Expansion**: Add digram tables for Russian, Chinese, and German.
- [ ] **Live Follow Mode**: Watch growing files or pipes in real-time.

## ⚖️ License
Distributed under the MIT License. See `LICENSE` for more information.
