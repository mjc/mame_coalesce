use std::collections::HashMap;
use std::io::{self, BufRead};

struct Sample {
    comm: String,
    tid: u32,
    time: f64,
    stack: Vec<String>,
}

fn main() {
    let stdin = io::stdin();
    let mut samples = Vec::new();
    let mut current_comm = String::new();
    let mut current_tid = 0;
    let mut current_time = 0.0;
    let mut current_stack = Vec::new();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(_) => continue,
        };

        if line.is_empty() {
            flush_sample(
                &mut samples,
                &mut current_comm,
                current_tid,
                current_time,
                &mut current_stack,
            );
            continue;
        }

        if !line.starts_with('\t') && !line.starts_with(' ') {
            if let Some((comm, tid, time)) = parse_header(&line) {
                current_comm = comm;
                current_tid = tid;
                current_time = time;
                current_stack.clear();
            }
        } else if let Some(function) = parse_frame(line.trim()) {
            current_stack.push(function);
        }
    }

    flush_sample(
        &mut samples,
        &mut current_comm,
        current_tid,
        current_time,
        &mut current_stack,
    );

    if samples.is_empty() {
        eprintln!("No samples parsed. Make sure to pipe `perf script` output.");
        std::process::exit(1);
    }

    print_thread_breakdown(&samples);
    println!();
    print_top_functions(&samples, 40);
    println!();
    print_top_functions_per_thread(&samples, 15);
    println!();
    print_callee_edges(&samples, 30);
    println!();
    print_timeline(&samples, 10);
    println!();
    print_category_summary(&samples);
}

fn flush_sample(
    samples: &mut Vec<Sample>,
    current_comm: &mut String,
    current_tid: u32,
    current_time: f64,
    current_stack: &mut Vec<String>,
) {
    if !current_comm.is_empty() && !current_stack.is_empty() {
        samples.push(Sample {
            comm: current_comm.clone(),
            tid: current_tid,
            time: current_time,
            stack: std::mem::take(current_stack),
        });
    }
    current_comm.clear();
    current_stack.clear();
}

fn parse_header(line: &str) -> Option<(String, u32, f64)> {
    let first_colon = line.find(':')?;
    let before_colon = line[..first_colon].trim_end();
    let ts_space = before_colon.rfind(' ')?;
    let time = before_colon[ts_space + 1..].parse().ok()?;
    let prefix = before_colon[..ts_space].trim_end();
    let prefix = if prefix.ends_with(']') {
        let bracket = prefix.rfind('[')?;
        prefix[..bracket].trim_end()
    } else {
        prefix
    };

    let last_space = prefix.rfind(' ')?;
    let comm = prefix[..last_space].trim().to_owned();
    let tid_str = &prefix[last_space + 1..];
    let tid = if let Some(slash) = tid_str.find('/') {
        tid_str[slash + 1..].parse().ok()?
    } else {
        tid_str.parse().ok()?
    };

    Some((comm, tid, time))
}

fn parse_frame(line: &str) -> Option<String> {
    let (_addr, rest) = line.split_once(' ')?;
    let function_part = if let Some(paren) = rest.rfind(" (") {
        &rest[..paren]
    } else {
        rest
    };

    let function = if let Some(plus) = function_part.rfind('+') {
        let after_plus = &function_part[plus + 1..];
        if after_plus.starts_with("0x") || after_plus.chars().all(|char| char.is_ascii_hexdigit()) {
            &function_part[..plus]
        } else {
            function_part
        }
    } else {
        function_part
    };

    if function.is_empty() {
        return None;
    }

    Some(function.to_owned())
}

fn print_thread_breakdown(samples: &[Sample]) {
    let total = samples.len();
    let mut by_thread: HashMap<(u32, &str), usize> = HashMap::new();
    for sample in samples {
        *by_thread.entry((sample.tid, &sample.comm)).or_insert(0) += 1;
    }

    let mut threads: Vec<_> = by_thread.into_iter().collect();
    threads.sort_by(|left, right| right.1.cmp(&left.1));

    println!("=== Thread Breakdown ({total} total samples) ===\n");
    println!("{:>7} {:>10}  {:>7}  comm", "%", "samples", "tid");
    println!("{}", "-".repeat(60));
    for ((tid, comm), count) in threads {
        let percent = count as f64 / total as f64 * 100.0;
        println!("{percent:>6.2}% {count:>10}  {tid:>7}  {comm}");
    }
}

fn print_top_functions(samples: &[Sample], n: usize) {
    let total = samples.len();
    let mut leaf_counts: HashMap<&str, usize> = HashMap::new();
    for sample in samples {
        if let Some(leaf) = sample.stack.first() {
            *leaf_counts.entry(leaf).or_insert(0) += 1;
        }
    }

    let mut functions: Vec<_> = leaf_counts.into_iter().collect();
    functions.sort_by(|left, right| right.1.cmp(&left.1));

    println!("=== Top {n} Functions (self/on-CPU time) ===\n");
    println!("{:>7} {:>10}  Function", "%", "samples");
    println!("{}", "-".repeat(100));

    let mut shown_percent = 0.0;
    for (function, count) in functions.iter().take(n) {
        let percent = *count as f64 / total as f64 * 100.0;
        println!("{percent:>6.2}% {count:>10}  {}", truncate(function, 80));
        shown_percent += percent;
    }

    println!("{}", "-".repeat(100));
    println!(
        "{shown_percent:>6.2}%             Total ({} functions shown)",
        functions.len().min(n)
    );
}

fn print_top_functions_per_thread(samples: &[Sample], n: usize) {
    let mut by_thread: HashMap<(u32, &str), Vec<&Sample>> = HashMap::new();
    for sample in samples {
        by_thread
            .entry((sample.tid, &sample.comm))
            .or_default()
            .push(sample);
    }

    let mut threads: Vec<_> = by_thread.into_iter().collect();
    threads.sort_by(|left, right| right.1.len().cmp(&left.1.len()));

    println!("=== Top Functions Per Thread ===");
    for ((tid, comm), thread_samples) in threads.iter().take(8) {
        let thread_total = thread_samples.len();
        let mut leaf_counts: HashMap<&str, usize> = HashMap::new();
        for sample in thread_samples {
            if let Some(leaf) = sample.stack.first() {
                *leaf_counts.entry(leaf).or_insert(0) += 1;
            }
        }

        let mut functions: Vec<_> = leaf_counts.into_iter().collect();
        functions.sort_by(|left, right| right.1.cmp(&left.1));

        println!("\n--- {comm} (tid {tid}, {thread_total} samples) ---\n");
        println!("{:>7} {:>10}  Function", "%", "samples");
        for (function, count) in functions.iter().take(n) {
            let percent = *count as f64 / thread_total as f64 * 100.0;
            println!("{percent:>6.2}% {count:>10}  {}", truncate(function, 75));
        }
    }
}

fn print_callee_edges(samples: &[Sample], n: usize) {
    let total = samples.len();
    let mut edges: HashMap<(&str, &str), usize> = HashMap::new();
    for sample in samples {
        for frames in sample.stack.windows(2) {
            let callee = frames[0].as_str();
            let caller = frames[1].as_str();
            *edges.entry((caller, callee)).or_insert(0) += 1;
        }
    }

    let mut edge_list: Vec<_> = edges.into_iter().collect();
    edge_list.sort_by(|left, right| right.1.cmp(&left.1));

    println!("=== Top {n} Caller -> Callee Edges ===\n");
    println!("{:>7} {:>10}  {:<50} -> Callee", "%", "samples", "Caller");
    println!("{}", "-".repeat(120));
    for ((caller, callee), count) in edge_list.iter().take(n) {
        let percent = *count as f64 / total as f64 * 100.0;
        println!(
            "{percent:>6.2}% {count:>10}  {:<50} -> {}",
            truncate(caller, 50),
            truncate(callee, 50)
        );
    }
}

fn print_timeline(samples: &[Sample], buckets: usize) {
    let min_time = samples
        .iter()
        .map(|sample| sample.time)
        .fold(f64::INFINITY, f64::min);
    let max_time = samples
        .iter()
        .map(|sample| sample.time)
        .fold(f64::NEG_INFINITY, f64::max);
    let duration = max_time - min_time;

    if duration <= 0.0 {
        println!("=== Timeline ===\n");
        println!("All samples are at the same timestamp; cannot bucket.");
        return;
    }

    let bucket_width = duration / buckets as f64;
    let mut bucket_vec: Vec<Bucket> = (0..buckets)
        .map(|index| Bucket {
            start: min_time + index as f64 * bucket_width,
            total: 0,
            by_thread: HashMap::new(),
            top_functions: HashMap::new(),
        })
        .collect();

    for sample in samples {
        let index = ((sample.time - min_time) / bucket_width) as usize;
        let bucket = &mut bucket_vec[index.min(buckets - 1)];
        bucket.total += 1;
        *bucket.by_thread.entry(sample.tid).or_insert(0) += 1;
        if let Some(leaf) = sample.stack.first() {
            *bucket.top_functions.entry(leaf.clone()).or_insert(0) += 1;
        }
    }

    let mut thread_totals: HashMap<u32, usize> = HashMap::new();
    let mut tid_to_comm: HashMap<u32, &str> = HashMap::new();
    for sample in samples {
        *thread_totals.entry(sample.tid).or_insert(0) += 1;
        tid_to_comm.entry(sample.tid).or_insert(&sample.comm);
    }
    let mut top_threads: Vec<_> = thread_totals.into_iter().collect();
    top_threads.sort_by(|left, right| right.1.cmp(&left.1));
    let top_threads: Vec<u32> = top_threads.iter().take(6).map(|(tid, _)| *tid).collect();

    println!("=== Timeline ({duration:.1}s duration, {buckets} buckets) ===\n");
    println!("Sample distribution over time, useful for cold vs hot phases.\n");
    print!("{:>12} {:>8}", "Time(s)", "Samples");
    for tid in &top_threads {
        let name = tid_to_comm.get(tid).copied().unwrap_or("?");
        print!("  {:>10}", truncate(name, 10));
    }
    println!("  Top function");
    println!("{}", "-".repeat(120));

    for bucket in &bucket_vec {
        let offset = bucket.start - min_time;
        print!(
            "{:>8.1}-{:<3.1} {:>8}",
            offset,
            offset + bucket_width,
            bucket.total
        );
        for tid in &top_threads {
            let count = bucket.by_thread.get(tid).copied().unwrap_or(0);
            let percent = if bucket.total > 0 {
                count as f64 / bucket.total as f64 * 100.0
            } else {
                0.0
            };
            print!("  {percent:>7.1}%  ");
        }
        if let Some((function, _count)) =
            bucket.top_functions.iter().max_by_key(|(_, count)| *count)
        {
            print!("  {}", truncate(function, 40));
        }
        println!();
    }
}

struct Bucket {
    start: f64,
    total: usize,
    by_thread: HashMap<u32, usize>,
    top_functions: HashMap<String, usize>,
}

fn print_category_summary(samples: &[Sample]) {
    let total = samples.len();
    let mut categories: HashMap<&str, usize> = HashMap::new();
    for sample in samples {
        if let Some(leaf) = sample.stack.first() {
            *categories.entry(categorize(leaf)).or_insert(0) += 1;
        }
    }

    let mut categories: Vec<_> = categories.into_iter().collect();
    categories.sort_by(|left, right| right.1.cmp(&left.1));

    println!("=== Category Summary (self time) ===\n");
    println!("{:>7} {:>10}  Category", "%", "samples");
    println!("{}", "-".repeat(40));
    for (category, count) in categories {
        let percent = count as f64 / total as f64 * 100.0;
        println!("{percent:>6.2}% {count:>10}  {category}");
    }
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

fn truncate(value: &str, max: usize) -> &str {
    if value.len() <= max {
        value
    } else {
        &value[..max]
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
