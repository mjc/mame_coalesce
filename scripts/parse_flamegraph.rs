use std::collections::{BTreeSet, HashMap};
use std::{env, fs, io};

struct Entry {
    name: String,
    samples: u64,
    percent: f64,
}

#[derive(Clone, Copy)]
struct Span {
    start: u64,
    end: u64,
}

struct Frame {
    name: String,
    span: Span,
    y: u64,
}

struct Profile {
    entries: Vec<Entry>,
    frames: Vec<Frame>,
    samples: u64,
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
    let profile = parse_profile(&content)?;

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
            cmd_top(&profile.entries, n, min_pct);
        }
        "search" => {
            let pattern = args.get(3).map(String::as_str).unwrap_or("");
            cmd_search(&profile.entries, pattern);
        }
        "syscalls" => cmd_syscalls(&profile),
        "summary" => cmd_summary(&profile),
        "diff" => {
            let Some(other_path) = args.get(3) else {
                eprintln!("Usage: {} <before.svg> diff <after.svg>", args[0]);
                std::process::exit(1);
            };
            let other_content = fs::read_to_string(other_path)?;
            let other_profile = parse_profile(&other_content)?;
            cmd_diff(&profile.entries, &other_profile.entries);
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
    eprintln!("  summary            Show category sample coverage");
    eprintln!("  diff <other.svg>   Compare matched-workload coverage");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  {program} flamegraph.svg top 20");
    eprintln!("  {program} flamegraph.svg search planner");
    eprintln!("  {program} flamegraph.svg syscalls");
    eprintln!("  {program} flamegraph.svg summary");
    eprintln!("  {program} before.svg diff after.svg");
}

fn parse_profile(content: &str) -> io::Result<Profile> {
    let mut frames = Vec::new();
    for chunk in content.split("<title>") {
        if let Some(end) = chunk.find("</title>") {
            let title = &chunk[..end];
            if let Some((name, _, _)) = parse_title(title) {
                let rect = chunk[end + "</title>".len()..]
                    .split_once("<rect ")
                    .and_then(|(_, rect)| rect.split_once('>').map(|(rect, _)| rect))
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            "flamegraph frame has no rectangle",
                        )
                    })?;
                let start = rect_attribute(rect, "fg:x")?;
                let width = rect_attribute(rect, "fg:w")?;
                let y = rect_attribute(rect, "y")?;
                let end = start.checked_add(width).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "flamegraph sample range overflow",
                    )
                })?;
                frames.push(Frame {
                    name,
                    span: Span { start, end },
                    y,
                });
            }
        }
    }
    let samples = frames
        .iter()
        .map(|frame| frame.span.end)
        .max()
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "no Inferno sample ranges found")
        })?;
    if samples == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "empty flamegraph",
        ));
    }
    let mut by_name: HashMap<&str, Vec<Span>> = HashMap::new();
    for frame in &frames {
        by_name.entry(&frame.name).or_default().push(frame.span);
    }
    let mut entries: Vec<_> = by_name
        .into_iter()
        .map(|(name, spans)| {
            let covered = covered_samples(&spans);
            Entry {
                name: name.to_owned(),
                samples: covered,
                percent: percentage(covered, samples),
            }
        })
        .collect();
    entries.sort_by(|left, right| {
        right
            .percent
            .partial_cmp(&left.percent)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(Profile {
        entries,
        frames,
        samples,
    })
}

fn rect_attribute(rect: &str, name: &str) -> io::Result<u64> {
    let marker = format!("{name}=\"");
    rect.split_once(&marker)
        .and_then(|(_, value)| value.split_once('"'))
        .and_then(|(value, _)| value.parse().ok())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("missing {name} sample range"),
            )
        })
}

fn covered_samples(spans: &[Span]) -> u64 {
    let mut spans = spans.to_vec();
    spans.sort_unstable_by_key(|span| span.start);
    let (mut covered, mut end) = (0, 0);
    for span in spans {
        if span.start >= end {
            covered += span.end - span.start;
            end = span.end;
        } else if span.end > end {
            covered += span.end - end;
            end = span.end;
        }
    }
    covered
}

fn percentage(covered: u64, total: u64) -> f64 {
    covered as f64 / total as f64 * 100.0
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
    println!("Top {n} functions by inclusive sample coverage (>= {min_pct:.1}%):\n");
    println!("{:>7} {:>10}  Function", "%", "samples");
    println!("{}", "-".repeat(90));

    let mut shown = 0;
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
        shown += 1;
    }
    println!("\n{shown} functions shown; ancestor and descendant coverage overlaps.");
}

fn cmd_search(entries: &[Entry], pattern: &str) {
    let pattern_lower = pattern.to_lowercase();
    println!("Functions matching '{pattern}' (inclusive coverage):\n");
    println!("{:>7} {:>10}  Function", "%", "samples");
    println!("{}", "-".repeat(90));

    let mut count = 0;
    for entry in entries {
        if entry.name.to_lowercase().contains(&pattern_lower) {
            println!(
                "{:>6.2}% {:>10}  {}",
                entry.percent,
                entry.samples,
                truncate_name(&entry.name, 65)
            );
            count += 1;
        }
    }

    println!("\n{count} matches; matching functions can cover the same samples.");
}

fn cmd_syscalls(profile: &Profile) {
    println!("Syscall frame coverage:\n");
    println!("{:>7}  Syscall", "%");
    println!("{}", "-".repeat(60));

    let mut spans = Vec::new();
    for entry in &profile.entries {
        if entry.name.starts_with("__x64_sys_") || entry.name.starts_with("__x86_sys_") {
            let syscall_name = entry
                .name
                .strip_prefix("__x64_sys_")
                .or_else(|| entry.name.strip_prefix("__x86_sys_"))
                .unwrap_or(&entry.name);
            println!("{:>6.2}%  {syscall_name}", entry.percent);
        }
    }
    for frame in &profile.frames {
        if frame.name.starts_with("__x64_sys_") || frame.name.starts_with("__x86_sys_") {
            spans.push(frame.span);
        }
    }
    println!("{}", "-".repeat(60));
    println!(
        "{:>6.2}%  Any syscall frame",
        percentage(covered_samples(&spans), profile.samples)
    );
}

fn category_coverage(profile: &Profile) -> Vec<(&'static str, u64)> {
    let mut events = Vec::with_capacity(profile.frames.len() * 2);
    let process_root = profile
        .frames
        .iter()
        .enumerate()
        .filter(|(_, frame)| frame.span.start == 0 && frame.span.end == profile.samples)
        .max_by_key(|(_, frame)| frame.y)
        .map(|(index, _)| index);
    for (index, frame) in profile.frames.iter().enumerate() {
        if Some(index) != process_root && frame.span.start < frame.span.end {
            events.push((frame.span.start, 1, index));
            events.push((frame.span.end, 0, index));
        }
    }
    events.sort_unstable();
    let mut active: BTreeSet<(u64, usize)> = BTreeSet::new();
    let mut categories: HashMap<&str, u64> = HashMap::new();
    let mut previous = 0;
    for (position, kind, index) in events {
        if position > previous {
            let category = active.iter().next().map_or("Other", |(_, index)| {
                categorize(&profile.frames[*index].name)
            });
            *categories.entry(category).or_default() += position - previous;
            previous = position;
        }
        let key = (profile.frames[index].y, index);
        if kind == 0 {
            active.remove(&key);
        } else {
            active.insert(key);
        }
    }
    if previous < profile.samples {
        let category = active.iter().next().map_or("Other", |(_, index)| {
            categorize(&profile.frames[*index].name)
        });
        *categories.entry(category).or_default() += profile.samples - previous;
    }
    let mut covered: Vec<_> = categories.into_iter().collect();
    covered.sort_unstable_by(|left, right| right.1.cmp(&left.1));
    covered
}

fn cmd_summary(profile: &Profile) {
    println!("Exclusive category samples (deepest visible frame; rows sum to 100%):\n");
    println!("{:>7} {:>12}  Category", "%", "samples");
    println!("{}", "-".repeat(40));
    for (category, covered) in category_coverage(profile) {
        println!(
            "{:>6.2}% {:>12}  {category}",
            percentage(covered, profile.samples),
            covered
        );
    }

    println!("\n{}", "=".repeat(60));
    println!("Key functions by category:\n");

    for category in categories_for_reports() {
        let functions: Vec<_> = profile
            .entries
            .iter()
            .filter(|entry| {
                categorize(&entry.name) == *category
                    && entry.percent >= 0.5
                    && !(*category == "Other" && entry.samples == profile.samples)
            })
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

    println!("Flamegraph diff: inclusive coverage, comparable workloads only\n");
    print_deltas("Functions with more coverage:", &regressions);
    print_deltas("Functions with less coverage:", &improvements);

    if regressions.is_empty() && improvements.is_empty() {
        println!("No significant differences found (threshold: 0.01%).");
    } else {
        println!(
            "{} functions changed; deltas overlap and must not be summed.",
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
        "Catalog import",
        "SQLite/Diesel",
        "DAT/XML",
        "JSON/Serde",
        "Hashing",
        "Archive/Compression",
        "Scan/Walk",
        "Planner",
        "Writer",
        "Rayon/Threading",
        "Disk I/O",
        "Memory",
        "Syscall",
        "Other",
    ]
}

fn categorize(name: &str) -> &'static str {
    let lower = name.to_lowercase();

    if contains_any(
        &lower,
        &[
            "storage::catalog_import",
            "storage::documents",
            "storage::relationships",
        ],
    ) {
        return "Catalog import";
    }
    if contains_any(&lower, &["serde_json", "serde_core", "serde::"]) {
        return "JSON/Serde";
    }
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
            "yy_reduce",
            "exprdup",
            "getrowtrigger",
            "coderowtrigger",
            "getpagenormal",
            "getandinitpage",
            "balance",
        ],
    ) {
        return "SQLite/Diesel";
    }
    if contains_any(
        &lower,
        &[
            "serde_xml_rs",
            "quick_xml",
            "xml_reader",
            "xml",
            "logiqx",
            "mame::parse",
            "mame_softwarelist::parse",
        ],
    ) {
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
    use super::{categorize, category_coverage, covered_samples, parse_profile, Span};

    #[test]
    fn unions_overlapping_and_repeated_flamegraph_frames() {
        let svg = concat!(
            "<title>catalog_import_ (100 samples, 100.00%)</title><rect y=\"100\" fg:x=\"0\" fg:w=\"100\"/>",
            "<title>sqlite3_prepare_v3 (40 samples, 40.00%)</title><rect y=\"80\" fg:x=\"0\" fg:w=\"40\"/>",
            "<title>sqlite3RunParser (40 samples, 40.00%)</title><rect y=\"60\" fg:x=\"0\" fg:w=\"40\"/>",
            "<title>sqlite3_prepare_v3 (40 samples, 40.00%)</title><rect y=\"40\" fg:x=\"20\" fg:w=\"40\"/>",
            "<title>sqlite3_step (20 samples, 20.00%)</title><rect y=\"20\" fg:x=\"50\" fg:w=\"20\"/>",
            "<title>quick_xml::reader (30 samples, 30.00%)</title><rect y=\"10\" fg:x=\"20\" fg:w=\"30\"/>",
        );
        let profile = parse_profile(svg).unwrap();
        assert_eq!(profile.samples, 100);
        assert_eq!(
            profile
                .entries
                .iter()
                .filter(|entry| entry.name == "sqlite3_prepare_v3")
                .count(),
            1
        );
        assert_eq!(
            profile
                .entries
                .iter()
                .find(|entry| entry.name == "sqlite3_prepare_v3")
                .unwrap()
                .samples,
            60
        );
        let categories = category_coverage(&profile);
        assert_eq!(
            categories
                .iter()
                .find(|(name, _)| *name == "SQLite/Diesel")
                .unwrap()
                .1,
            40
        );
        assert_eq!(
            categories
                .iter()
                .find(|(name, _)| *name == "DAT/XML")
                .unwrap()
                .1,
            30
        );
        assert_eq!(
            categories
                .iter()
                .find(|(name, _)| *name == "Other")
                .unwrap()
                .1,
            30
        );
        assert_eq!(
            categories.iter().map(|(_, count)| count).sum::<u64>(),
            profile.samples
        );
    }

    #[test]
    fn rejects_missing_inferno_sample_ranges() {
        let svg = "<title>sqlite3_step (1 samples, 100.00%)</title><rect x=\"0%\" width=\"100%\"/>";
        assert!(parse_profile(svg).is_err());
    }

    #[test]
    fn counts_disjoint_intervals_once() {
        assert_eq!(
            covered_samples(&[
                Span { start: 0, end: 10 },
                Span { start: 20, end: 30 },
                Span { start: 5, end: 25 }
            ]),
            30
        );
    }

    #[test]
    fn categorizes_catalog_import_symbols() {
        assert_eq!(categorize("sqlite3LockAndPrepare"), "SQLite/Diesel");
        assert_eq!(categorize("yy_reduce.isra.0"), "SQLite/Diesel");
        assert_eq!(categorize("exprDup"), "SQLite/Diesel");
        assert_eq!(categorize("getRowTrigger"), "SQLite/Diesel");
        assert_eq!(categorize("getPageNormal"), "SQLite/Diesel");
        assert_eq!(categorize("mame_coalesce::xml_reader::next"), "DAT/XML");
        assert_eq!(categorize("serde_json::to_value"), "JSON/Serde");
        assert_eq!(
            categorize("mame_coalesce::storage::catalog_import::import"),
            "Catalog import"
        );
    }

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
