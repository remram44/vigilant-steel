use log::{
    Level, LevelFilter, Log, Metadata, Record, SetLoggerError, set_logger,
    set_max_level,
};

#[link(wasm_import_module = "mod")]
extern {
    fn log_str(a: *const u8, len: usize);
}

struct JsLogger;

impl Log for JsLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let s = format!("{} - {}", record.target(), record.args());
            unsafe {
                log_str(s.as_ptr(), s.len());
            }
        }
    }

    fn flush(&self) {}
}

pub fn init(level: LevelFilter) -> Result<(), SetLoggerError> {
    set_max_level(level);
    set_logger(&JsLogger)
}
