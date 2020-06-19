use log::{
    Level, LevelFilter, Log, Metadata, Record, SetLoggerError, set_logger,
    set_max_level,
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    fn log_str(s: &str);
}

struct JsLogger;

impl Log for JsLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let s = format!("{} - {}", record.target(), record.args());
            log_str(&s);
        }
    }

    fn flush(&self) {}
}

pub fn init(level: LevelFilter) -> Result<(), SetLoggerError> {
    set_max_level(level);
    set_logger(&JsLogger)
}
