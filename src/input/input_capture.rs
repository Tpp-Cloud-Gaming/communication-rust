use std::io::{Error, ErrorKind};
use std::sync::Arc;

use webrtc::data_channel::RTCDataChannel;
use webrtc::peer_connection::RTCPeerConnection;
use winput::message_loop;
use winput::message_loop::EventReceiver;
use winput::{Action, Vk};

use crate::utils::shutdown;

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
            match pc.create_data_channel("KEYBOARD", None).await {
                Ok(ch) => ch,
                Err(_) => {
                    return Err(Error::new(
                        ErrorKind::Other,
                        "Error creating latency data channel",
                    ))
                }
            };
        let mouse_channel: Arc<RTCDataChannel> = match pc.create_data_channel("MOUSE", None).await {
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
                    println!("{:?} was pressed!", vk);
                }
            }
            message_loop::Event::MouseMoveRelative { x, y } => {
                println!("Mouse moved relative: x: {}, y: {}", x, y);
                //TODO: obviamente revisar esto, es solo prueba y se tiene que validar que el channel este abierto primero
                mouse_channel
                    .send_text(std::format!("{} {}", x, y).as_str())
                    .await
                    .unwrap();
                //mouse_channel.send(&[x as u8, y as u8]).await.unwrap();
            }
            _ => (),
        }
    }
}
