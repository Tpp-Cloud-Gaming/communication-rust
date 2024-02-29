
use std::io::{Error, ErrorKind};
use std::sync::Arc;

use webrtc::data_channel::RTCDataChannel;
use webrtc::peer_connection::RTCPeerConnection;
use winput::Mouse;
use webrtc::data_channel::data_channel_message::DataChannelMessage;

pub struct MouseController {
}

impl MouseController {
    pub fn new() -> MouseController {
        MouseController {
            
        }
    }

    pub fn start_mouse_controller(ch: Arc<RTCDataChannel>) {
        //println!("Mouse controller started");
        ch.on_message(Box::new(move |msg: DataChannelMessage| {
          
            //println!("{:?}", msg.data);
            let s = String::from_utf8_lossy(&msg.data);

             // Split the string into two parts
            let parts: Vec<&str> = s.split_whitespace().collect();

            // Parse the parts into integers
            let x = parts[0].parse::<i32>().unwrap();
            let y = parts[1].parse::<i32>().unwrap();

            //println!("x: {}, y: {}", x, y);
            // Use the parsed integers in the move_relative function
            Mouse::move_relative(x, y);
            

            Box::pin(async {})
        }));
    }
}