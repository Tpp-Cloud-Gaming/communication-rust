use std::sync::Arc;

use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::RTCDataChannel;
use winput::Mouse;

/// # MouseController
///
/// The `MouseController` struct provides functionality for handling keyboard and mouse events
/// via a WebRTC data channel
pub struct MouseController {}

impl MouseController {
    /// Creates a new `MouseController`.
    pub fn new() -> MouseController {
        MouseController {}
    }

    /// Starts the mouse controller by registering a callback for incoming messages on the
    /// provided WebRTC data channel.
    ///
    /// # Arguments
    ///
    /// * `ch` - An Arc reference to the RTCDataChannel.
    pub fn start_mouse_controller(ch: Arc<RTCDataChannel>) {
        ch.on_message(Box::new(move |msg: DataChannelMessage| {
            Box::pin(async move {
                let s = String::from_utf8_lossy(&msg.data);

                // Split the string into two parts
                let parts: Vec<&str> = s.split_whitespace().collect();

                // Parse the parts into integers
                let x = match parts[0].parse::<i32>() {
                    Ok(x) => x,
                    Err(e) => {
                        log::error!("MOUSE CONTROLLER | Error parsing i32: {}", e);
                        return;
                    }
                };
                let y = match parts[1].parse::<i32>() {
                    Ok(y) => y,
                    Err(e) => {
                        log::error!("MOUSE CONTROLLER | Error parsing i32: {}", e);
                        return;
                    }
                };

                //thread::sleep(std::time::Duration::from_micros(MOUSE_DELAY));
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
