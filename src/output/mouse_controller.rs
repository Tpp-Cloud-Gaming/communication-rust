use std::sync::Arc;
use std::thread;

use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::RTCDataChannel;
use winput::Mouse;

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

                thread::sleep(std::time::Duration::from_micros(500));
                Mouse::move_relative(x, y);

                // let step_size = 1;
                // let x_steps = ((x.abs() + step_size - 1) / step_size).max(1);
                // let y_steps = ((y.abs() + step_size - 1) / step_size).max(1);

                // let x_step = if x != 0 { x / x_steps } else { 0 };
                // let y_step = if y != 0 { y / y_steps } else { 0 };

                // for _ in 0..x_steps.max(y_steps) {
                //     Mouse::move_relative(x_step, y_step);
                //     thread::sleep(std::time::Duration::from_micros(500));
                // }
            })
        }));
    }
}
