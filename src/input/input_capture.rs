use std::io::{Error, ErrorKind};
use std::str::Bytes;
use std::sync::Arc;

use webrtc::data_channel::RTCDataChannel;
use webrtc::peer_connection::RTCPeerConnection;
use winput::message_loop::EventReceiver;
use winput::{message_loop, Button};
use winput::{Action, Vk};

use crate::utils::shutdown;

use super::input_const::{KEYBOARD_CHANNEL_LABEL, MOUSE_CHANNEL_LABEL};

pub struct InputCapture {
    shutdown: shutdown::Shutdown,
    keyboard_channel: Arc<RTCDataChannel>,
    mouse_channel: Arc<RTCDataChannel>,
}

impl InputCapture {
    pub async fn new(
        pc: Arc<RTCPeerConnection>,
        shutdown: shutdown::Shutdown,
    ) -> Result<InputCapture, Error> {
        let keyboard_channel: Arc<RTCDataChannel> =
            match pc.create_data_channel(KEYBOARD_CHANNEL_LABEL, None).await {
                Ok(ch) => ch,
                Err(_) => {
                    return Err(Error::new(
                        ErrorKind::Other,
                        "Error creating latency data channel",
                    ))
                }
            };
        let mouse_channel: Arc<RTCDataChannel> =
            match pc.create_data_channel(MOUSE_CHANNEL_LABEL, None).await {
                Ok(ch) => ch,
                Err(_) => {
                    return Err(Error::new(
                        ErrorKind::Other,
                        "Error creating latency data channel",
                    ))
                }
            };

        Ok(InputCapture {
            shutdown,
            keyboard_channel,
            mouse_channel,
        })
    }

    pub async fn start(&self) -> Result<(), Error> {
        self.shutdown.add_task().await;

        println!("Starting");
        let receiver: EventReceiver = match message_loop::start() {
            Ok(receiver) => receiver,
            Err(e) => {
                return Err(Error::new(
                    ErrorKind::Other,
                    "Error setting local description",
                ))
            }
        };

        tokio::select! {
            _ = self.shutdown.wait_for_error() => {
                message_loop::stop();
            }
            _= start_handler(receiver, self.keyboard_channel.clone(), self.mouse_channel.clone()) => {
                message_loop::stop();
                println!("Stopped");

            }
        }
        return Ok(());
    }
}

async fn start_handler(
    receiver: EventReceiver,
    keyboard_channel: Arc<RTCDataChannel>,
    mouse_channel: Arc<RTCDataChannel>,
) {
    loop {
        match receiver.next_event() {
            message_loop::Event::Keyboard {
                vk,
                action: Action::Press,
                ..
            } => {
                if vk == Vk::Escape {
                    break;
                } else {
                    if keyboard_channel.ready_state()
                        == webrtc::data_channel::data_channel_state::RTCDataChannelState::Open
                    {
                        let keyboard_channel_cpy = keyboard_channel.clone();
                        tokio::task::spawn(async move {
                            let vk_txt = vk.into_u8().to_string();
                            keyboard_channel_cpy
                                .send_text(std::format!("p{}", vk_txt).as_str())
                                .await
                                .unwrap();
                        });
                    }
                }
            }
            message_loop::Event::Keyboard {
                vk,
                action: Action::Release,
                ..
            } => {
                if keyboard_channel.ready_state()
                    == webrtc::data_channel::data_channel_state::RTCDataChannelState::Open
                {
                    let keyboard_channel_cpy = keyboard_channel.clone();
                    tokio::task::spawn(async move {
                        let vk_txt = vk.into_u8().to_string();
                        keyboard_channel_cpy
                            .send_text(std::format!("r{}", vk_txt).as_str())
                            .await
                            .unwrap();
                    });
                }
            }
            message_loop::Event::MouseButton {
                action: Action::Press,
                button,
            } => {
                let vk = match button {
                    Button::Left => "Left",
                    Button::Right => "Right",
                    Button::Middle => "Middle",
                    Button::X1 => "X1",
                    Button::X2 => "X2",
                };
                let keyboard_channel_cpy = keyboard_channel.clone();
                if keyboard_channel.ready_state()
                    == webrtc::data_channel::data_channel_state::RTCDataChannelState::Open
                {
                    tokio::task::spawn(async move {
                        keyboard_channel_cpy
                            .send_text(std::format!("m{}", vk).as_str())
                            .await
                            .unwrap();
                    });
                }
            }
            message_loop::Event::MouseButton {
                action: Action::Release,
                button,
            } => {
                let vk = match button {
                    Button::Left => 0,
                    Button::Right => 1,
                    Button::Middle => 2,
                    Button::X1 => 3,
                    Button::X2 => 4,
                };
                let keyboard_channel_cpy = keyboard_channel.clone();
                if keyboard_channel.ready_state()
                    == webrtc::data_channel::data_channel_state::RTCDataChannelState::Open
                {
                    tokio::task::spawn(async move {
                        keyboard_channel_cpy
                            .send_text(std::format!("t{}", vk).as_str())
                            .await
                            .unwrap();
                    });
                }
            }
            message_loop::Event::MouseMoveRelative { x, y } => {
                //println!("Mouse moved relative: x: {}, y: {}", x, y);
                //TODO: chequea que este en abierto en canal (ver si es la mejor forma), se podria validar caso de error aca?, ver si conviene trasmitir bytes en vez de texto, sacar unwraps
                if x == 0 && y == 0 {
                    continue;
                }
                if mouse_channel.ready_state()
                    == webrtc::data_channel::data_channel_state::RTCDataChannelState::Open
                {
                    let mouse_channel_cpy = mouse_channel.clone();
                    tokio::task::spawn(async move {
                        mouse_channel_cpy
                            .send_text(std::format!("{} {}", x, y).as_str())
                            .await
                            .unwrap();
                    });
                } else {
                    //println!("Mouse channel is not open");
                }
            }
            _ => (),
        }
    }
}
