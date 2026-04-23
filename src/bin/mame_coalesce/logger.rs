use log::LevelFilter;
use simplelog::{CombinedLogger, TermLogger};

pub fn setup() {
    if let Err(error) = CombinedLogger::init(vec![TermLogger::new(
        LevelFilter::Info,
        simplelog::Config::default(),
        simplelog::TerminalMode::Mixed,
        simplelog::ColorChoice::Never,
    )]) {
        eprintln!("Unable to start logger: {error}");
    }
}
