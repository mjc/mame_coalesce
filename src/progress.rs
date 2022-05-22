use indicatif::{ProgressBar, ProgressStyle};

fn bar_style() -> ProgressStyle {
    ProgressStyle::default_bar()
        .template("[{elapsed}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg} ETA: {eta}")
}

pub fn bar(len: u64) -> ProgressBar {
    let bar = ProgressBar::new(len);
    bar.set_style(bar_style());
    bar
}
