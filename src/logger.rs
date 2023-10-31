use log::LevelFilter;
use simplelog::{CombinedLogger, TermLogger};

pub fn setup() {
    let logger_result = CombinedLogger::init(vec![TermLogger::new(
        LevelFilter::Info,
        simplelog::Config::default(),
        simplelog::TerminalMode::Mixed,
        simplelog::ColorChoice::Never,
    )]);
    assert!(logger_result.is_ok(), "Unable to start logger");
}
