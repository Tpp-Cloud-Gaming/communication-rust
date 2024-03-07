use super::output_const::*;
use std::mem;
use std::sync::Arc;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::RTCDataChannel;
use winapi::um::winuser::*;
use winput::Button;

pub struct ButtonController {}

impl ButtonController {
    pub fn new() -> ButtonController {
        ButtonController {}
    }

    pub fn start_keyboard_controller(ch: Arc<RTCDataChannel>) {
        ch.on_message(Box::new(move |msg: DataChannelMessage| {
            Box::pin(async move {
                let s = String::from_utf8_lossy(&msg.data);
                let (action, rest) = s.split_at(1);
                let key = rest.parse::<u8>().unwrap();

                //let (action, key) = get_action_and_key(&msg.data);
                match action {
                    PRESS_KEYBOARD_ACTION => {
                        send_input_key(key as i32, false);
                    }
                    RELEASE_KEYBOARD_ACTION => {
                        send_input_key(key as i32, true);
                    }
                    PRESS_MOUSE_ACTION => {
                        let button = get_mouse_button(key);
                        winput::press(button);
                    }
                    RELEASE_MOUSE_ACTION => {
                        let button = get_mouse_button(key);
                        winput::release(button);
                    }
                    _ => {}
                }
            })
        }));
    }
}

fn get_mouse_button(key: u8) -> Button {
    match key {
        0 => Button::Left,
        1 => Button::Right,
        2 => Button::Middle,
        3 => Button::X1,
        4 => Button::X2,
        _ => Button::Left, //TODO: fix this
    }
}

pub fn send_input_key(virtual_key: i32, up: bool) {
    unsafe {
        let mut input = INPUT {
            type_: INPUT_KEYBOARD,
            u: std::mem::zeroed(),
        };
        *input.u.ki_mut() = KEYBDINPUT {
            wVk: virtual_key as u16,
            dwFlags: if up { KEYEVENTF_KEYUP } else { 0 },
            dwExtraInfo: 1,
            wScan: KEYEVENTF_SCANCODE as u16,
            time: 0,
        };

        SendInput(1, &mut input, mem::size_of::<INPUT>() as i32);
    }
}
