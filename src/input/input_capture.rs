use std::io::{Error, ErrorKind};
use std::sync::Arc;
use webrtc::data_channel::RTCDataChannel;
use webrtc::peer_connection::RTCPeerConnection;
use winput::message_loop::EventReceiver;
use winput::{message_loop, Button};
use winput::{Action, Vk};

use super::input_const::{KEYBOARD_CHANNEL_LABEL, MOUSE_CHANNEL_LABEL};
use crate::output::output_const::*;
use crate::utils::shutdown;

/// # InputCapture
///
/// The `InputCapture` struct represents a mechanism for capturing input events and send them via WebRTC data channels.
pub struct InputCapture {
    shutdown: shutdown::Shutdown,
    button_channel: Arc<RTCDataChannel>,
    mouse_channel: Arc<RTCDataChannel>,
}

impl InputCapture {
    /// Creates a new `InputCapture`.
    ///
    /// # Arguments
    ///
    /// * `pc` - An Arc reference to the RTCPeerConnection.
    /// * `shutdown` - A shutdown handle for managing the finalization of the thread.
    pub async fn new(
        pc: Arc<RTCPeerConnection>,
        shutdown: &mut shutdown::Shutdown,
    ) -> Result<InputCapture, Error> {
        let button_channel: Arc<RTCDataChannel> =
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

        let shutdown_cpy = shutdown.clone();
        Ok(InputCapture {
            shutdown: shutdown_cpy,
            button_channel,
            mouse_channel,
        })
    }

    pub async fn start(&mut self) -> Result<(), Error> {
        self.shutdown.add_task("Input Capture").await;

        let receiver: EventReceiver = match message_loop::start() {
            Ok(receiver) => receiver,
            Err(_e) => {
                return Err(Error::new(
                    ErrorKind::Other,
                    "Error setting local description",
                ))
            }
        };

        tokio::select! {
            _ = self.shutdown.wait_for_error() => {
                log::info!("INPUT CAPTURE | Shutdown received");
                message_loop::stop();
            }
            _ = start_handler(receiver, self.button_channel.clone(), self.mouse_channel.clone(),self.shutdown.clone()) => {
                message_loop::stop();

            }
        }
        Ok(())
    }
}

/// Starts the input handler by listening for input events and sending them through the data channels.
///
/// # Arguments
///
/// * `receiver` - An EventReceiver for listening to input events.
/// * `button_channel` - An Arc reference to the RTCDataChannel for the keyboard.
/// * `mouse_channel` - An Arc reference to the RTCDataChannel for the mouse.
/// * `shutdown` - A shutdown handle for managing the finalization of the thread.
///
/// # Returns
///
/// A Result containing () if the operation was successful, otherwise an Error is returned.
async fn start_handler(
    receiver: EventReceiver,
    button_channel: Arc<RTCDataChannel>,
    mouse_channel: Arc<RTCDataChannel>,
    shutdown: shutdown::Shutdown,
) {
    let shutdown_cpy = shutdown.clone();

    // The List of keys that will be blocked by the APP:
    let block_keys = vec![Vk::LeftWin, Vk::RightWin];

    loop {
        let shutdown_cpy_loop = shutdown_cpy.clone();

        tokio::task::spawn(async move {});

        match receiver.next_event() {
            message_loop::Event::Keyboard {
                vk,
                action: Action::Press,
                scan_code: _,
            } => {
                if block_keys.contains(&vk) {
                    continue;
                };

                let button_channel_cpy = button_channel.clone();
                let key = vk.into_u8().to_string();
                match handle_button_action(
                    button_channel_cpy,
                    PRESS_KEYBOARD_ACTION,
                    key,
                    shutdown_cpy_loop.clone(),
                )
                .await
                {
                    Ok(_) => (),
                    Err(e) => eprintln!("Failed to handle button action: {}", e),
                }
            }
            message_loop::Event::Keyboard {
                vk,
                action: Action::Release,
                scan_code: _,
            } => {
                if block_keys.contains(&vk) {
                    continue;
                };

                let button_channel_cpy = button_channel.clone();
                let key = vk.into_u8().to_string();
                match handle_button_action(
                    button_channel_cpy,
                    RELEASE_KEYBOARD_ACTION,
                    key,
                    shutdown_cpy_loop.clone(),
                )
                .await
                {
                    Ok(_) => (),
                    Err(e) => eprintln!("Failed to handle button action: {}", e),
                }
            }
            message_loop::Event::MouseButton {
                action: Action::Press,
                button,
            } => {
                let button_channel_cpy = button_channel.clone();
                match handle_button_action(
                    button_channel_cpy,
                    PRESS_MOUSE_ACTION,
                    button_to_i32(button).to_string(),
                    shutdown_cpy_loop.clone(),
                )
                .await
                {
                    Ok(_) => (),
                    Err(e) => eprintln!("Failed to handle button action: {}", e),
                }
            }
            message_loop::Event::MouseButton {
                action: Action::Release,
                button,
            } => {
                let button_channel_cpy = button_channel.clone();

                match handle_button_action(
                    button_channel_cpy,
                    RELEASE_MOUSE_ACTION,
                    button_to_i32(button).to_string(),
                    shutdown_cpy_loop.clone(),
                )
                .await
                {
                    Ok(_) => (),
                    Err(e) => eprintln!("Failed to handle button action: {}", e),
                }
            }
            message_loop::Event::MouseMoveRelative { x, y } => {
                if x == 0 && y == 0 {
                    continue;
                }
                if mouse_channel.ready_state()
                    == webrtc::data_channel::data_channel_state::RTCDataChannelState::Open
                {
                    let mouse_channel_cpy = mouse_channel.clone();
                    tokio::task::spawn(async move {
                        match mouse_channel_cpy
                            .send_text(std::format!("{} {}", x, y).as_str())
                            .await
                        {
                            Ok(_) => (),
                            Err(e) => eprintln!("Failed to send mouse event: {}", e),
                        }
                    });
                }
            }
            _ => (),
        }

        if shutdown.check_for_error().await {
            log::info!("INPUT CAPTURE | Shutdown received on check for error");
            break;
        };
    }
}

/// Handles the button action by sending the corresponding message through the data channel.
///
/// # Arguments
///     
/// * `button_channel` - An Arc reference to the RTCDataChannel.
/// * `action` - A string slice representing the action to be performed.
/// * `text` - A string representing the text to be sent.
/// * `shutdown` - A shutdown handle for managing the finalization of the thread.
///
/// # Returns
///
/// A Result containing () if the operation was successful, otherwise an Error is returned.
async fn handle_button_action(
    button_channel: Arc<RTCDataChannel>,
    action: &str,
    text: String,
    shutdown: shutdown::Shutdown,
) -> Result<(), Error> {
    let action_clone = action.to_owned();
        if button_channel.ready_state()
            == webrtc::data_channel::data_channel_state::RTCDataChannelState::Open
        {
            if let Err(_e) = button_channel
                .send_text(std::format!("{}{}", action_clone, text).as_str())
                .await
            {
                shutdown.notify_error(false, "Button action channel").await;
                return Err(Error::new(
                    ErrorKind::Other,
                    "Error sending message through data channel",
                ));
            };
        };
        Ok(())
}

/// Maps the mouse button to the corresponding integer value.
///
/// # Arguments
///
/// * `button` - A Button value.
///
/// # Returns
///
/// An integer value corresponding to the button.
fn button_to_i32(button: Button) -> i32 {
    match button {
        Button::Left => 0,
        Button::Right => 1,
        Button::Middle => 2,
        Button::X1 => 3,
        Button::X2 => 4,
    }
}
