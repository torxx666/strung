//! Strung: A high-performance, intelligent string extraction tool.
//! 
//! Designed as a modern replacement for the classic `strings` utility,
//! Strung uses linguistic analysis and deobfuscation to filter out binary noise.

use clap::Parser;
use memmap2::MmapOptions;
use object::{Object, ObjectSection};
use rayon::prelude::*;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::path::Path;
use base64::{prelude::BASE64_STANDARD, Engine};

mod digrams;
use digrams::{ENGLISH_DIGRAMS, SPANISH_DIGRAMS, FRENCH_DIGRAMS};

// --- Models ---

/// Command-line arguments for Strung.
#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Skip junk filtering (behave like standard strings)
    #[arg(short, long)]
    all: bool,

    /// Silent mode: do not log dropped strings
    #[arg(short, long)]
    silent: bool,

    /// Set the minimum string length (default: 4)
    #[arg(short = 'n', long, default_value_t = 4)]
    min_len: usize,

    /// Path to a config file (TOML)
    #[arg(short, long)]
    config: Option<String>,

    /// Generate a default config file (strung.toml)
    #[arg(long)]
    generate_config: bool,

    /// Disable duplicate removal
    #[arg(long)]
    no_duplicate: bool,

    /// Disable significance sorting
    #[arg(long)]
    no_sort: bool,

    /// Debug mode: output <string> <count> <score> [origin]
    #[arg(short, long)]
    debug: bool,

    /// Minimum significance threshold (0.0 to 1.0)
    #[arg(short = 'g', long = "gt")]
    threshold: Option<f32>,

    /// Sort order: l (length), p (probability), n (count)
    #[arg(short = 'o', long = "order")]
    order: Option<String>,

    /// Show offset: d (decimal), x (hex)
    #[arg(short = 't', long = "show-offset")]
    offset: Option<String>,

    /// Regex pattern to match strings
    #[arg(short = 'm', long = "match")]
    regex: Option<String>,

    /// Encoding: s (single-byte), l (little-endian 16-bit), b (big-endian 16-bit)
    #[arg(short = 'e', long = "encoding", default_value = "s")]
    encoding: String,

    /// Context: number of bytes to show around the string
    #[arg(short = 'C', long = "context", default_value_t = 0)]
    context: usize,

    /// Smart Mode: highlight high-significance strings with terminal colors
    #[arg(short = 'z', long)]
    smart: bool,

    /// Scan specific binary section (e.g., .rodata, .text)
    #[arg(short = 'S', long = "section")]
    section: Option<String>,

    /// Entropy Analysis: report regions of high/low entropy with visual sparklines
    #[arg(long)]
    entropy: bool,

    /// Base64 Auto-Decoding: transparently detect and decode Base64 strings
    #[arg(long)]
    base64: bool,

    /// XOR Brute-force: try every single-byte XOR key on rejected data
    #[arg(long)]
    xor: bool,

    /// Input file to scan
    #[arg(index = 1, required_unless_present = "generate_config")]
    file: Option<String>,
}

/// Persistent configuration for filtering and output.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct Config {
    #[serde(default = "default_true")] pub no_number: bool,
    #[serde(default = "default_true")] pub mix_cons_voy: bool,
    #[serde(default = "default_rx_len")] pub rx_effective_length: usize,
    #[serde(default = "default_true")] pub rx_symbole: bool,
    #[serde(default = "default_true")] pub english_filter: bool,
    #[serde(default = "default_true")] pub remove_duplicate: bool,
    #[serde(default = "default_true")] pub sort_significance: bool,
    #[serde(default = "default_false")] pub debug: bool,
    #[serde(default = "default_zero_f32")] pub threshold: f32,
    #[serde(default = "default_sort_by")] pub sort_by: String,
    pub show_offset: Option<String>,
    pub match_regex: Option<String>,
    #[serde(default = "default_encoding")] pub encoding: String,
    #[serde(default = "default_zero_usize")] pub context: usize,
    pub section: Option<String>,
    #[serde(default = "default_false")] pub smart: bool,
    #[serde(default = "default_false")] pub entropy: bool,
    #[serde(default = "default_false")] pub base64: bool,
    #[serde(default = "default_false")] pub xor: bool,
}

fn default_true() -> bool { true }
fn default_false() -> bool { false }
fn default_rx_len() -> usize { 10 }
fn default_zero_f32() -> f32 { 0.0 }
fn default_zero_usize() -> usize { 0 }
fn default_sort_by() -> String { "p".to_string() }
fn default_encoding() -> String { "s".to_string() }

impl Default for Config {
    fn default() -> Self {
        Self {
            no_number: true, mix_cons_voy: true, rx_effective_length: 10,
            rx_symbole: true, english_filter: true, remove_duplicate: true,
            sort_significance: true, debug: false, threshold: 0.0,
            sort_by: "p".to_string(), show_offset: None, match_regex: None,
            encoding: "s".to_string(), context: 0, section: None,
            smart: false, entropy: false, base64: false, xor: false,
        }
    }
}

/// Represents a found string with its assigned significance score and metadata.
#[derive(Debug, Clone)]
struct ScoredString {
    content: String,
    score: f32,
    offset: u64,
    prefix_context: Vec<u8>,
    suffix_context: Vec<u8>,
    origin: String,
    is_secret: bool,
}

// --- Entropy Analysis ---

/// Calculate Shannon entropy of a byte slice. Range: 0.0 (uniform) to 8.0 (random).
fn calculate_entropy(data: &[u8]) -> f32 {
    if data.is_empty() { return 0.0; }
    let mut counts = [0usize; 256];
    for &b in data { counts[b as usize] += 1; }
    let len = data.len() as f32;
    let mut entropy = 0.0;
    for &count in counts.iter() {
        if count > 0 {
            let p = count as f32 / len;
            entropy -= p * p.log2();
        }
    }
    entropy
}

/// Return a UTF-8 sparkline character representing the entropy level.
fn get_entropy_sparkline(h: f32) -> String {
    let chars = [" ", " ", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
    let idx = ((h / 8.0) * (chars.len() - 1) as f32).round() as usize;
    let idx = std::cmp::min(idx, chars.len() - 1);
    chars[idx].to_string()
}

// --- Filtering & Scoring Engine ---

fn is_vowel(c: char) -> bool {
    matches!(c.to_ascii_lowercase(), 'a' | 'e' | 'i' | 'o' | 'u' | 'y')
}

/// Check if a string contains at least one common digram (letter pair) for supported languages.
fn check_digram(s_lower: &str, table: &[(&str, f32)]) -> bool {
    if s_lower.len() < 2 { return true; }
    for i in 0..s_lower.len() - 1 {
        if let Some(pair) = s_lower.get(i..i+2) {
            if table.iter().any(|(d, _)| *d == pair) {
                return true;
            }
        }
    }
    false
}

/// Heuristic check: does this string look like natural language (English, Spanish, or French)?
fn is_likely_meaningful(s: &str) -> bool {
    let s_lower = s.to_lowercase();
    check_digram(&s_lower, ENGLISH_DIGRAMS) || 
    check_digram(&s_lower, SPANISH_DIGRAMS) || 
    check_digram(&s_lower, FRENCH_DIGRAMS)
}

/// Main scoring function. Combines length, linguistic balance, and symbol density.
fn get_significance_score(s: &str) -> f32 {
    let len = s.len();
    if len == 0 { return 0.0; }
    
    let mut score = 0.0;
    let s_lower = s.to_lowercase();

    // Reward human-like characteristics
    if len >= 6 {
        let mixed_case = s.chars().any(|c| c.is_lowercase()) && s.chars().any(|c| c.is_uppercase());
        let has_space = s.contains(' ');
        if mixed_case { score += 0.01; }
        if has_space { score += 0.01; }
    }

    // Heavy reward for keywords (URLs, common words)
    let has_common_word = ["the", "and", "ing", "tion", "http", "www", "com", "net", "org"].iter().any(|&w| s_lower.contains(w));
    if has_common_word { score += 0.02; }
    
    // Smooth digram density scoring
    let mut pair_count = 0;
    let mut total_freq = 0.0;
    for i in 0..s_lower.len() - 1 {
        if let Some(pair) = s_lower.get(i..i+2) {
            if let Some((_, freq)) = ENGLISH_DIGRAMS.iter().find(|(d, _)| *d == pair) {
                total_freq += freq;
                pair_count += 1;
            }
        }
    }
    if pair_count > 0 { score += (total_freq / pair_count as f32) * 0.05; }

    // Penalize high symbol density (typical of binary garbage)
    let symbol_count = s.chars().filter(|c| !c.is_alphanumeric() && *c != ' ').count() as f32;
    let symbol_ratio = symbol_count / len as f32;
    if len < 8 { score -= symbol_ratio * 0.1; } else { score -= symbol_ratio * 0.05; }
    
    score = score.max(0.0);
    if len >= 10 { score += (len as f32 / 1000.0).min(0.05); }
    score
}

/// Decide if a string is binary noise or potentially useful text.
fn is_not_junk(s: &str, config: &Config) -> bool {
    let len = s.len();
    // Rule: Don't show strings that are purely digits (unless -a is used)
    if config.no_number && s.chars().all(|c| c.is_ascii_digit()) { return false; }
    // Rule: Strings must have at least one vowel to look like a word
    if config.mix_cons_voy && !s.chars().any(is_vowel) { return false; }
    // Rule: Linguistic digram filter
    if config.english_filter && len < config.rx_effective_length && !is_likely_meaningful(s) { return false; }
    // Rule: Reject high-symbol strings
    if !config.rx_symbole && len < config.rx_effective_length {
        if s.chars().any(|c| !c.is_alphanumeric() && c != ' ') { return false; }
    }
    let alpha_count = s.chars().filter(|c| c.is_ascii_alphabetic()).count();
    let symbol_count = s.chars().filter(|c| !c.is_alphanumeric() && *c != ' ').count();
    if len < config.rx_effective_length && symbol_count > 0 && alpha_count < symbol_count + (len / 4) { return false; }
    true
}

// --- Scanning Infrastructure ---

/// Entry point for scanning a chunk of data. Handles raw and XOR variants.
fn scan_batch(
    data: &[u8], base_offset: u64, args: &Args, config: &Config, compiled_regex: &Option<Regex>,
) -> Vec<ScoredString> {
    let mut results = Vec::new();
    // Raw scan (normal strings)
    results.extend(perform_scan(data, base_offset, args, config, compiled_regex, 0, "raw"));
    // Brute-force XOR scan (if enabled)
    if config.xor {
        for key in 1..256 {
            results.extend(perform_scan(data, base_offset, args, config, compiled_regex, key as u8, &format!("xor:0x{:02x}", key)));
        }
    }
    results
}

/// Internal iterator that reconstructs strings from bytes based on encoding.
fn perform_scan(
    data: &[u8], base_offset: u64, args: &Args, config: &Config, compiled_regex: &Option<Regex>, xor_key: u8, origin: &str,
) -> Vec<ScoredString> {
    let mut results = Vec::new();
    let mut current_bytes = Vec::new();
    let encoding = config.encoding.as_str();
    let context_len = config.context;
    let mut prev_byte: Option<u8> = None;

    for (i, &raw_b) in data.iter().enumerate() {
        let b = raw_b ^ xor_key;
        let abs_offset = base_offset + i as u64;

        let char_found = match encoding {
            "l" => if let Some(p) = prev_byte { // UTF-16 LE heuristic
                if b == 0x00 && (p.is_ascii_graphic() || p == b' ') { let c = p; prev_byte = None; Some(c) }
                else { prev_byte = Some(b); None }
            } else { prev_byte = Some(b); None },
            "b" => if let Some(p) = prev_byte { // UTF-16 BE heuristic
                if p == 0x00 && (b.is_ascii_graphic() || b == b' ') { let c = b; prev_byte = None; Some(c) }
                else { prev_byte = Some(b); None }
            } else { prev_byte = Some(b); None },
            _ => if b.is_ascii_graphic() || b == b' ' { Some(b) } else { None }
        };

        if let Some(c) = char_found { current_bytes.push(c); } else {
            if !((encoding == "l" || encoding == "b") && prev_byte.is_some()) {
                if current_bytes.len() >= args.min_len {
                    let s_len_bytes = if encoding == "s" { current_bytes.len() as u64 } else { (current_bytes.len() * 2) as u64 };
                    let start_offset = abs_offset - s_len_bytes;
                    if let Ok(s) = String::from_utf8(current_bytes.clone()) {
                        process_string(s, start_offset, data, base_offset, context_len, &mut results, args, config, compiled_regex, origin);
                    }
                }
                current_bytes.clear();
            }
        }
    }
    results
}

/// Detect Indicator of Compromise (IOC) patterns like AWS IDs or API keys.
fn is_secret(s: &str) -> bool {
    let s_clean = s.trim();
    if s_clean.len() < 16 { return false; }
    
    // AWS Access Key ID
    if s_clean.contains("AKIA") {
        let re = Regex::new(r"AKIA[0-9A-Z]{12,20}").unwrap();
        if re.is_match(s_clean) { return true; }
    }
    
    // Generic high-entropy key detection (Hex or Base64-like)
    if s_clean.len() >= 32 && s_clean.chars().all(|c| c.is_ascii_hexdigit()) { return true; }

    let generic_key = Regex::new(r"(?i)(key|secret|token|password|passwd)[\s:=]{1,3}").unwrap();
    if generic_key.is_match(s) { return true; }

    false
}

/// Validates, scores, and optionally de-obfuscates (Base64) a candidate string.
fn process_string(
    s: String, start_offset: u64, data: &[u8], base_offset: u64, context_len: usize,
    results: &mut Vec<ScoredString>, args: &Args, config: &Config, compiled_regex: &Option<Regex>, origin: &str,
) {
    let matches_regex = compiled_regex.as_ref().map_or(true, |re| re.is_match(&s));
    if matches_regex && (args.all || is_not_junk(&s, config)) {
        let mut score = get_significance_score(&s);
        let secret = is_secret(&s);
        if secret { score += 0.8; } 

        if score >= config.threshold {
            let p_end = (start_offset - base_offset) as usize;
            let p_start = p_end.saturating_sub(context_len);
            let prefix = if context_len > 0 && p_end <= data.len() { data[p_start..p_end].to_vec() } else { Vec::new() };
            let s_len = if config.encoding == "s" { s.len() as u64 } else { (s.len() * 2) as u64 };
            let s_start_in_buf = (start_offset + s_len - base_offset) as usize;
            let s_end_in_buf = std::cmp::min(s_start_in_buf + context_len, data.len());
            let suffix = if context_len > 0 && s_end_in_buf > s_start_in_buf { data[s_start_in_buf..s_end_in_buf].to_vec() } else { Vec::new() };

            results.push(ScoredString {
                content: s.clone(), score, offset: start_offset, prefix_context: prefix,
                suffix_context: suffix, origin: origin.to_string(), is_secret: secret,
            });

            // Base64 auto-decoding
            if config.base64 && s.len() >= 8 && s.chars().all(|c| c.is_alphanumeric() || c == '+' || c == '/' || c == '=') {
                if let Ok(decoded) = BASE64_STANDARD.decode(s.trim()) {
                    if let Ok(ds) = String::from_utf8(decoded) {
                         if ds.len() >= args.min_len && is_not_junk(&ds, config) {
                            results.push(ScoredString {
                                content: format!("(B64: {})", ds), score: get_significance_score(&ds) + 0.1,
                                offset: start_offset, prefix_context: Vec::new(), suffix_context: Vec::new(),
                                origin: "base64".to_string(), is_secret: is_secret(&ds),
                            });
                         }
                    }
                }
            }
        }
    }
}

// --- Main Engine ---

fn main() -> io::Result<()> {
    let args = Args::parse();
    if args.generate_config { generate_config_template(); return Ok(()); }
    let config = load_and_merge_config(&args)?;
    let compiled_regex = config.match_regex.as_ref().map(|r| Regex::new(r).expect("Invalid Regex"));
    let input_path = args.file.as_ref().expect("Input file required");
    let file = File::open(input_path)?;
    let mmap = unsafe { MmapOptions::new().map(&file)? };

    // Entropy Analysis phase
    if config.entropy {
        println!("--- Entropy Analysis ---");
        let chunk_size = 4096;
        let mut i = 0;
        while i < mmap.len() {
            let end = std::cmp::min(i + chunk_size, mmap.len());
            let chunk = &mmap[i..end];
            let h = calculate_entropy(chunk);
            let spark = get_entropy_sparkline(h);
            let max_possible = (chunk.len() as f32).log2().min(8.0);
            if h > max_possible * 0.9 || h > 7.2 { println!("{:8x}: |{}| {:.4} [HIGH]", i, spark, h); }
            else if h < 1.0 { println!("{:8x}: |{}| {:.4} [LOW]", i, spark, h); }
            i += chunk_size;
        }
        println!("------------------------\n");
    }

    // Determine scan regions (all file or specific section)
    let mut scan_regions = Vec::new();
    if let Some(target_section) = &config.section {
        if let Ok(obj) = object::File::parse(&*mmap) {
            for section in obj.sections() {
                if let Ok(name) = section.name() {
                    if name == target_section {
                        if let Ok(data) = section.data() {
                            let (file_offset, _) = section.file_range().unwrap_or((0, 0));
                            scan_regions.push((data, file_offset));
                        }
                    }
                }
            }
        }
    } else {
        let chunk_size = 1024 * 1024;
        let overlap = 1024; // Ensure we don't miss strings crossing chunk boundaries
        let mut start = 0;
        while start < mmap.len() {
            let end = std::cmp::min(start + chunk_size + overlap, mmap.len());
            scan_regions.push((&mmap[start..end], start as u64));
            if end == mmap.len() { break; }
            start += chunk_size;
        }
    }

    // Parallel execution across CPU cores
    let chunk_results: Vec<Vec<ScoredString>> = scan_regions.par_iter()
        .map(|(data, offset)| scan_batch(data, *offset, &args, &config, &compiled_regex))
        .collect();

    let mut string_counts = HashMap::new();
    let mut unique_strings = HashMap::new();
    let mut results_order = Vec::new();
    let mut total_extracted = 0;

    // Deduplication and aggregation
    for batch in chunk_results {
        for ss in batch {
            total_extracted += 1;
            let count = string_counts.entry(ss.content.clone()).or_insert(0);
            if *count == 0 {
                results_order.push(ss.content.clone());
                unique_strings.insert(ss.content.clone(), ss);
            }
            *count += 1;
        }
    }

    // Sorting strategy
    if config.sort_significance {
        let sort_by = config.sort_by.clone();
        results_order.sort_by(|a, b| match sort_by.as_str() {
            "l" => a.len().cmp(&b.len()),
            "n" => string_counts.get(a).unwrap().cmp(string_counts.get(b).unwrap()),
            _ => unique_strings.get(a).unwrap().score.partial_cmp(&unique_strings.get(b).unwrap().score).unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    // Final output
    let mut total_outputted = 0;
    for s_name in results_order {
        let count = string_counts[&s_name];
        let ss = &unique_strings[&s_name];
        if config.remove_duplicate { print_string(ss, count, &config); total_outputted += 1; }
        else { for _ in 0..count { print_string(ss, count, &config); total_outputted += 1; } }
    }
    eprintln!("\n--- Summary ---");
    eprintln!("File scanned: {} | Extracted: {} | Outputted: {}", input_path, total_extracted, total_outputted);
    Ok(())
}

/// Print a single result with color highlighting and optional hex context.
fn print_string(ss: &ScoredString, count: usize, config: &Config) {
    let mut prefix = if let Some(ref fmt) = config.show_offset { if fmt == "x" { format!("{:8x} ", ss.offset) } else { format!("{:8} ", ss.offset) } } else { "".to_string() };
    if config.context > 0 {
        let p_hex: Vec<String> = ss.prefix_context.iter().map(|b| format!("{:02x}", b)).collect();
        let s_hex: Vec<String> = ss.suffix_context.iter().map(|b| format!("{:02x}", b)).collect();
        prefix = format!("{}[{}]...[{}] ", prefix, p_hex.join(""), s_hex.join(""));
    }
    let mut content = ss.content.clone();
    let reset = "\x1b[0m";
    if config.smart {
        let (gr, b_gr, b_bl, mag, cyan, red) = ("\x1b[32m", "\x1b[1;32m", "\x1b[1;34m", "\x1b[35m", "\x1b[36m", "\x1b[1;31m");
        let ipv4_re = Regex::new(r"\b(?:[0-9]{1,3}\.){3}[0-9]{1,3}\b").unwrap();
        if ss.is_secret { content = format!("{}{}{}", red, content, reset); }
        else if content.contains("://") || content.contains("www.") || content.contains("@") || ipv4_re.is_match(&content) { content = format!("{}{}{}", b_bl, content, reset); }
        else if ss.origin == "base64" { content = format!("{}{}{}", mag, content, reset); }
        else if ss.origin.starts_with("xor") { content = format!("{}{}{}", cyan, content, reset); }
        else if ss.content.len() >= 6 {
            if ss.score > 0.05 { content = format!("{}{}{}", b_gr, content, reset); }
            else if ss.score > 0.02 { content = format!("{}{}{}", gr, content, reset); }
        }
    }
    if config.debug {
        let origin = if ss.origin != "raw" { format!(" [{}]", ss.origin) } else { "".to_string() };
        println!("{}{}{}\t{}\t{:.6}", prefix, content, origin, count, ss.score);
    } else { println!("{}{}", prefix, content); }
}

/// Merge CLI arguments into the default configuration.
fn load_and_merge_config(args: &Args) -> io::Result<Config> {
    let mut config = if let Some(path) = &args.config { toml::from_str(&std::fs::read_to_string(path)?).expect("Invalid config") }
    else if Path::new("strung.toml").exists() { toml::from_str(&std::fs::read_to_string("strung.toml")?).expect("Invalid config") }
    else { Config::default() };
    if args.no_duplicate { config.remove_duplicate = false; }
    if args.no_sort { config.sort_significance = false; }
    if args.debug { config.debug = true; }
    if let Some(t) = args.threshold { config.threshold = t; }
    if let Some(ref o) = args.order { config.sort_by = o.to_string(); config.sort_significance = true; }
    if let Some(ref t) = args.offset { config.show_offset = Some(t.to_string()); }
    if let Some(ref m) = args.regex { config.match_regex = Some(m.to_string()); }
    if args.encoding != "s" { config.encoding = args.encoding.clone(); }
    if args.context > 0 { config.context = args.context; }
    if let Some(ref s) = args.section { config.section = Some(s.clone()); }
    if args.smart { config.smart = true; config.sort_significance = true; }
    if args.entropy { config.entropy = true; }
    if args.base64 { config.base64 = true; }
    if args.xor { config.xor = true; }
    Ok(config)
}

/// Output a default TOML configuration to stdout.
fn generate_config_template() {
    println!(r#"# Strung Professional Configuration
no_number = true
mix_cons_voy = true
english_filter = true
rx_effective_length = 10
rx_symbole = true
remove_duplicate = true
sort_significance = true
sort_by = "p"
debug = false
threshold = 0.0
# show_offset = "x"
# match_regex = ".*"
# encoding = "s"
# context = 0
# section = ".rodata"
# smart = true
# entropy = true
# base64 = true
# xor = true
"#);
}
