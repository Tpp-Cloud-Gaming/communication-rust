use std::sync::Arc;

use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::RTCDataChannel;
use winput::{Action, Button, Mouse, Vk};

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
                let (action, rest) = s.split_at(1);

                let key = rest.parse::<u8>().unwrap();
                //println!("Key received {:?}", msg.data);

                if action == 'p'.to_string() {
                    winput::press(unsafe { Vk::from_u8(key) });

                    println!("recibo un presionar {:?}", key);
                } else if action == 'r'.to_string() {
                    winput::release(unsafe { Vk::from_u8(key) });
                    println!("recibo un soltar {:?}", key);
                } else {
                    let button = match key {
                        0 => Button::Left,
                        1 => Button::Right,
                        2 => Button::Middle,
                        3 => Button::X1,
                        4 => Button::X2,
                        _ => Button::Left, //TODO: fix this
                    };
                    if action == 'm'.to_string() {
                        winput::press(button);
                    } else if action == 't'.to_string() {
                        winput::release(button);
                    }
                }
            })
        }));
    }
}
