use std::sync::Arc;
use std::thread;

use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::RTCDataChannel;
use winput::Mouse;

use super::output_const::MOUSE_DELAY;

pub struct MouseController {}

impl MouseController {
    pub fn new() -> MouseController {
        MouseController {}
    }

    pub fn start_mouse_controller(ch: Arc<RTCDataChannel>) {
        //println!("Mouse controller started");
        ch.on_message(Box::new(move |msg: DataChannelMessage| {
            Box::pin(async move {
                //println!("{:?}", msg.data);

                let s = String::from_utf8_lossy(&msg.data);

                // Split the string into two parts
                let parts: Vec<&str> = s.split_whitespace().collect();

                // Parse the parts into integers
                let x = parts[0].parse::<i32>().unwrap();
                let y = parts[1].parse::<i32>().unwrap();

                thread::sleep(std::time::Duration::from_micros(MOUSE_DELAY));
                Mouse::move_relative(x, y);
            })
        }));
    }
}

impl Default for MouseController {
    fn default() -> Self {
        Self::new()
    }
}
