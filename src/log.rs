use log::{LevelFilter, Log, Metadata, Record, SetLoggerError};
use std::io::Write;

#[derive(Clone)]
pub struct Logger {
    log_level: LevelFilter,
}

impl Logger {
    pub fn register(log_level: LevelFilter) -> Result<(), SetLoggerError> {
        log::set_max_level(log_level);

        let boxed = Box::new(Self { log_level });
        let avg_rust_user = Box::leak(boxed);

        // We must leak the object to satisfy the 'static lifetime
        // constraint
        log::set_logger(avg_rust_user)
    }
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.log_level
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        eprintln!(
            "[PID {}] {}:{} -- {}",
            std::process::id(),
            record.level(),
            record.target(),
            record.args()
        );
    }

    fn flush(&self) {
        std::io::stderr().flush().ok();
    }
}
