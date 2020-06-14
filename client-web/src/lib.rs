#[macro_use]
extern crate log;

mod logger;

#[no_mangle]
pub extern fn init() {
    logger::init(log::LevelFilter::Info).unwrap();
}

#[no_mangle]
pub extern fn update() -> i32 {
    info!("test");
    42
}
