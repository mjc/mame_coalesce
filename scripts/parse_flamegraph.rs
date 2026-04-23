use std::collections::HashMap;
use std::{env, fs, io};

struct Entry {
    name: String,
    samples: u64,
    percent: f64,
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage(&args[0]);
        std::process::exit(1);
    }

    let svg_path = &args[1];
    let command = args.get(2).map(String::as_str).unwrap_or("top");
    let content = fs::read_to_string(svg_path)?;
    let entries = parse_entries(&content);

    match command {
        "top" => {
            let n = args
                .get(3)
                .and_then(|value| value.parse().ok())
                .unwrap_or(30);
            let min_pct = args
                .get(4)
                .and_then(|value| value.parse().ok())
                .unwrap_or(1.0);
            cmd_top(&entries, n, min_pct);
        }
        "search" => {
            let pattern = args.get(3).map(String::as_str).unwrap_or("");
            cmd_search(&entries, pattern);
        }
        "syscalls" => cmd_syscalls(&entries),
        "summary" => cmd_summary(&entries),
        "diff" => {
            let Some(other_path) = args.get(3) else {
                eprintln!("Usage: {} <before.svg> diff <after.svg>", args[0]);
                std::process::exit(1);
            };
            let other_content = fs::read_to_string(other_path)?;
            let other_entries = parse_entries(&other_content);
            cmd_diff(&entries, &other_entries);
        }
        _ => {
            eprintln!("Unknown command: {command}");
            print_usage(&args[0]);
            std::process::exit(1);
        }
    }

    Ok(())
}

fn print_usage(program: &str) {
    eprintln!("Usage: {program} <flamegraph.svg> [command] [args...]");
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  top [N] [min%]     Show top N functions (default: 30, min: 1.0%)");
    eprintln!("  search <pattern>   Search for functions matching pattern");
    eprintln!("  syscalls           Show syscall breakdown");
    eprintln!("  summary            Show categorized summary");
    eprintln!("  diff <other.svg>   Compare two flamegraphs");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  {program} flamegraph.svg top 20");
    eprintln!("  {program} flamegraph.svg search planner");
    eprintln!("  {program} flamegraph.svg syscalls");
    eprintln!("  {program} flamegraph.svg summary");
    eprintln!("  {program} before.svg diff after.svg");
}

fn parse_entries(content: &str) -> Vec<Entry> {
    let mut results = Vec::new();

    for chunk in content.split("<title>") {
        if let Some(end) = chunk.find("</title>") {
            let title = &chunk[..end];
            if let Some((name, samples, percent)) = parse_title(title) {
                results.push(Entry {
                    name,
                    samples,
                    percent,
                });
            }
        }
    }

    results.sort_by(|left, right| {
        right
            .percent
            .partial_cmp(&left.percent)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results
}

fn parse_title(title: &str) -> Option<(String, u64, f64)> {
    let paren_start = title.rfind('(')?;
    let name = html_unescape(title[..paren_start].trim());
    let meta = &title[paren_start + 1..];
    let samples_end = meta.find(" samples")?;
    let samples_str = meta[..samples_end].replace(',', "");
    let samples = samples_str.parse().ok()?;
    let pct_start = meta.rfind(", ")? + 2;
    let pct_end = meta.rfind('%')?;
    let percent = meta[pct_start..pct_end].parse().ok()?;

    if name.is_empty() || name == "all" {
        return None;
    }

    Some((name, samples, percent))
}

fn html_unescape(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn cmd_top(entries: &[Entry], n: usize, min_pct: f64) {
    println!("Top {n} functions (>= {min_pct:.1}%):\n");
    println!("{:>7} {:>10}  Function", "%", "samples");
    println!("{}", "-".repeat(90));

    let mut shown = 0;
    let mut total = 0.0;
    for entry in entries {
        if entry.percent < min_pct {
            continue;
        }
        if shown >= n {
            break;
        }

        println!(
            "{:>6.2}% {:>10}  {}",
            entry.percent,
            entry.samples,
            truncate_name(&entry.name, 65)
        );
        total += entry.percent;
        shown += 1;
    }

    println!("{}", "-".repeat(90));
    println!("{total:>6.2}%             Total ({shown} functions shown)");
}

fn cmd_search(entries: &[Entry], pattern: &str) {
    let pattern_lower = pattern.to_lowercase();
    println!("Functions matching '{pattern}':\n");
    println!("{:>7} {:>10}  Function", "%", "samples");
    println!("{}", "-".repeat(90));

    let mut total = 0.0;
    let mut count = 0;
    for entry in entries {
        if entry.name.to_lowercase().contains(&pattern_lower) {
            println!(
                "{:>6.2}% {:>10}  {}",
                entry.percent,
                entry.samples,
                truncate_name(&entry.name, 65)
            );
            total += entry.percent;
            count += 1;
        }
    }

    println!("{}", "-".repeat(90));
    println!("{total:>6.2}%             Total ({count} matches)");
}

fn cmd_syscalls(entries: &[Entry]) {
    println!("Syscall breakdown:\n");
    println!("{:>7}  Syscall", "%");
    println!("{}", "-".repeat(60));

    let mut total = 0.0;
    for entry in entries {
        if entry.name.starts_with("__x64_sys_") || entry.name.starts_with("__x86_sys_") {
            let syscall_name = entry
                .name
                .strip_prefix("__x64_sys_")
                .or_else(|| entry.name.strip_prefix("__x86_sys_"))
                .unwrap_or(&entry.name);
            println!("{:>6.2}%  {syscall_name}", entry.percent);
            total += entry.percent;
        }
    }

    println!("{}", "-".repeat(60));
    println!("{total:>6.2}%  Total syscall time");
}

fn cmd_summary(entries: &[Entry]) {
    let mut categories: HashMap<&str, f64> = HashMap::new();
    for entry in entries {
        *categories.entry(categorize(&entry.name)).or_insert(0.0) += entry.percent;
    }

    let mut categories: Vec<_> = categories.into_iter().collect();
    categories.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    println!("Category summary:\n");
    println!("{:>7}  Category", "%");
    println!("{}", "-".repeat(40));
    for (category, percent) in &categories {
        println!("{percent:>6.2}%  {category}");
    }

    println!("\n{}", "=".repeat(60));
    println!("Key functions by category:\n");

    for category in categories_for_reports() {
        let functions: Vec<_> = entries
            .iter()
            .filter(|entry| categorize(&entry.name) == *category && entry.percent >= 0.5)
            .take(5)
            .collect();

        if !functions.is_empty() {
            println!("{category}:");
            for entry in functions {
                println!(
                    "  {:>5.2}%  {}",
                    entry.percent,
                    truncate_name(&entry.name, 55)
                );
            }
            println!();
        }
    }
}

fn cmd_diff(before: &[Entry], after: &[Entry]) {
    let before_map: HashMap<&str, (u64, f64)> = before
        .iter()
        .map(|entry| (entry.name.as_str(), (entry.samples, entry.percent)))
        .collect();
    let after_map: HashMap<&str, (u64, f64)> = after
        .iter()
        .map(|entry| (entry.name.as_str(), (entry.samples, entry.percent)))
        .collect();

    let mut names = Vec::new();
    for entry in before {
        names.push(entry.name.as_str());
    }
    for entry in after {
        if !before_map.contains_key(entry.name.as_str()) {
            names.push(entry.name.as_str());
        }
    }

    let mut deltas = Vec::new();
    for name in names {
        let (before_samples, before_pct) = before_map.get(name).copied().unwrap_or((0, 0.0));
        let (after_samples, after_pct) = after_map.get(name).copied().unwrap_or((0, 0.0));
        let diff_pct = after_pct - before_pct;
        if diff_pct.abs() >= 0.01 {
            deltas.push(Delta {
                name,
                before_pct,
                after_pct,
                diff_pct,
                before_samples,
                after_samples,
            });
        }
    }

    deltas.sort_by(|left, right| {
        right
            .diff_pct
            .abs()
            .partial_cmp(&left.diff_pct.abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let regressions: Vec<_> = deltas.iter().filter(|delta| delta.diff_pct > 0.0).collect();
    let improvements: Vec<_> = deltas.iter().filter(|delta| delta.diff_pct < 0.0).collect();

    println!("Flamegraph diff: before vs after\n");
    print_deltas("REGRESSIONS (gained CPU):", &regressions);
    print_deltas("IMPROVEMENTS (lost CPU):", &improvements);

    if regressions.is_empty() && improvements.is_empty() {
        println!("No significant differences found (threshold: 0.01%).");
    } else {
        let total_regression: f64 = regressions.iter().map(|delta| delta.diff_pct).sum();
        let total_improvement: f64 = improvements.iter().map(|delta| delta.diff_pct).sum();
        println!(
            "Summary: {total_regression:>+.2}% regressions, {total_improvement:>+.2}% improvements ({} functions changed)",
            deltas.len()
        );
    }
}

fn print_deltas(title: &str, deltas: &[&Delta<'_>]) {
    if deltas.is_empty() {
        return;
    }

    println!("{title}\n");
    println!(
        "{:>8} {:>8} {:>8}  {:>10} {:>10}  Function",
        "before%", "after%", "delta%", "before_n", "after_n"
    );
    println!("{}", "-".repeat(100));
    for delta in deltas.iter().take(30) {
        println!(
            "{:>7.2}% {:>7.2}% {:>+7.2}%  {:>10} {:>10}  {}",
            delta.before_pct,
            delta.after_pct,
            delta.diff_pct,
            delta.before_samples,
            delta.after_samples,
            truncate_name(delta.name, 42)
        );
    }
    println!();
}

struct Delta<'a> {
    name: &'a str,
    before_pct: f64,
    after_pct: f64,
    diff_pct: f64,
    before_samples: u64,
    after_samples: u64,
}

fn categories_for_reports() -> &'static [&'static str] {
    &[
        "Hashing",
        "Archive/Compression",
        "Scan/Walk",
        "Planner",
        "Writer",
        "SQLite/Diesel",
        "DAT/XML",
        "Rayon/Threading",
        "Disk I/O",
        "Memory",
        "Syscall",
        "Other",
    ]
}

fn categorize(name: &str) -> &'static str {
    let lower = name.to_lowercase();

    if contains_any(&lower, &["sha1", "sha1_asm", "xxh3", "xxhash", "digest"]) {
        return "Hashing";
    }
    if contains_any(
        &lower,
        &[
            "operations::scan",
            "walkdir",
            "infer",
            "scan_zip",
            "scan_7z",
            "scan_rar",
            "walk_for_files",
        ],
    ) {
        return "Scan/Walk";
    }
    if contains_any(
        &lower,
        &[
            "build::planner",
            "plan_build",
            "sources_for_root",
            "selected_dat_roms",
            "btreemap",
        ],
    ) {
        return "Planner";
    }
    if contains_any(
        &lower,
        &[
            "build::writer",
            "zipwriter",
            "copy_from_archive",
            "copy_from_archive_entry",
            "copy_from_zip_entry",
            "copy_from_7z_archive",
            "copy_from_rar_archive",
            "copy_bare_file",
        ],
    ) {
        return "Writer";
    }
    if contains_any(
        &lower,
        &[
            "zip",
            "r7z",
            "unrar",
            "compress_tools",
            "libarchive",
            "inflate",
            "deflate",
            "zstd",
            "bzip",
            "lzma",
            "decompress",
        ],
    ) {
        return "Archive/Compression";
    }
    if contains_any(
        &lower,
        &[
            "diesel",
            "sqlite",
            "libsqlite3",
            "storage::repositories",
            "storage::db",
        ],
    ) {
        return "SQLite/Diesel";
    }
    if contains_any(&lower, &["serde_xml_rs", "xml", "logiqx"]) {
        return "DAT/XML";
    }
    if contains_any(&lower, &["rayon", "crossbeam", "thread_pool"]) {
        return "Rayon/Threading";
    }
    if contains_any(
        &lower,
        &[
            "read", "write", "pread", "pwrite", "open", "vfs", "ext4", "xfs", "btrfs", "zfs",
            "io_uring",
        ],
    ) {
        return "Disk I/O";
    }
    if contains_any(
        &lower,
        &[
            "alloc", "malloc", "free", "rawvec", "realloc", "memcpy", "memmove", "mmap", "brk",
        ],
    ) {
        return "Memory";
    }
    if name.starts_with("__x64_sys_")
        || name.starts_with("syscall")
        || name.starts_with("do_syscall")
        || name.starts_with("entry_SYSCALL")
    {
        return "Syscall";
    }

    "Other"
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn truncate_name(name: &str, max_len: usize) -> String {
    if name.len() <= max_len {
        name.to_owned()
    } else {
        format!("{}...", &name[..max_len - 3])
    }
}

#[cfg(test)]
mod tests {
    use super::categorize;

    #[test]
    fn categorizes_current_archive_libraries() {
        assert_eq!(
            categorize("r7z::Archive::stream_files"),
            "Archive/Compression"
        );
        assert_eq!(
            categorize("unrar::archive::OpenArchive::read_header"),
            "Archive/Compression"
        );
        assert_eq!(
            categorize("zip::read::read_zipfile_from_stream"),
            "Archive/Compression"
        );
    }

    #[test]
    fn categorizes_app_scan_and_writer_before_archive_libraries() {
        assert_eq!(
            categorize("mame_coalesce::operations::scan::scan_zip"),
            "Scan/Walk"
        );
        assert_eq!(
            categorize("mame_coalesce::operations::scan::scan_rar"),
            "Scan/Walk"
        );
        assert_eq!(
            categorize("mame_coalesce::build::writer::copy_from_zip_entry"),
            "Writer"
        );
        assert_eq!(
            categorize("mame_coalesce::build::writer::copy_from_rar_archive"),
            "Writer"
        );
    }
}
