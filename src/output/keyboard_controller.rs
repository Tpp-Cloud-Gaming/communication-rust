use std::sync::Arc;

use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::RTCDataChannel;
use winput::{Action, Vk};

pub struct KeyboardController {}

impl KeyboardController {
    pub fn new() -> KeyboardController {
        KeyboardController {}
    }

    pub fn start_keyboard_controller(ch: Arc<RTCDataChannel>) {
        //println!("Mouse controller started");
        ch.on_message(Box::new(move |msg: DataChannelMessage| {
            Box::pin(async move {
                let s = String::from_utf8_lossy(&msg.data);
                let (action, key) = s.split_at(0);

                if action == 'p'.to_string() {
                    println!("recibo un presionar");
                } else if action == 'r'.to_string() {
                    println!("recibo un soltar");
                }
            })
        }));
    }
}
