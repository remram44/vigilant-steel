use game::net::{Client, Message};
use log::warn;
use std::sync::mpsc::Sender;
use wasm_bindgen::prelude::*;

use super::get_app;

#[wasm_bindgen]
extern "C" {
    fn send_message(msg: &[u8]);
}

#[wasm_bindgen]
pub extern "C" fn recv_message(msg: Box<[u8]>) {
    if let Some(app) = get_app() {
        if let Some(recvq) = &app.recvq {
            match Message::parse(&msg) {
                Some(msg) => recvq.send(msg).unwrap(),
                None => warn!("Invalid message from server: {:?}", msg),
            }
        }
    }
}

pub fn setup() -> (impl Client, Sender<Message>) {
    game::net::stub::StubClient::new(
        |msg| -> Result<(), Box<dyn std::error::Error>> {
            send_message(&msg.bytes());
            Ok(())
        }
    )
}
