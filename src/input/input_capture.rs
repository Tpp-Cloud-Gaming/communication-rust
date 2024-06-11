use std::ffi::OsStr;
use std::io::{Error, ErrorKind};
use std::os::windows::ffi::OsStrExt;
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;
use std::{iter, ptr};
use webrtc::data_channel::RTCDataChannel;
use webrtc::peer_connection::RTCPeerConnection;
use winapi::um::{libloaderapi, winuser};
use winput::message_loop::{EventReceiver, MessageLoopError};
use winput::{message_loop, Button, WheelDirection, WindowsError};
use winput::{Action, Vk};

use super::input_const::{KEYBOARD_CHANNEL_LABEL, MOUSE_CHANNEL_LABEL};
use crate::output::output_const::*;
use crate::services::receiver;
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

        if message_loop::is_active() {
            println!("MSG LOOP | YA ESTA ACTIVO");
        }
        let receiver: EventReceiver = match message_loop::start() {
            Ok(receiver) => receiver,
            Err(MessageLoopError::AlreadyActive) => {
                return Err(Error::new(
                    ErrorKind::Other,
                    "INPUT CAPTURE | Failed to start: Already active",
                ))
            }
            Err(MessageLoopError::OsError(e)) => {
                return Err(Error::new(
                    ErrorKind::Other,
                    std::format!("INPUT CAPTURE | Failed to start: Os Error {}", e),
                ))
            }
        };

        tokio::select! {
            _ = self.shutdown.wait_for_error() => {
                log::info!("INPUT CAPTURE | Shutdown received");
            }
            _ = start_handler(receiver, self.button_channel.clone(), self.mouse_channel.clone(),self.shutdown.clone()) => {

            }
        }

        unregister_class_w();

        Ok(())
    }
}

// Unregister the class created by the message loop
// This is necessary to start the message loop again
fn unregister_class_w() {
    let mut attempts = 0;

    while attempts < 5 {
        // Retreives the module handle of the application.
        unsafe {
            let h_instance = libloaderapi::GetModuleHandleW(ptr::null());

            // Create the window.
            let class_name = OsStr::new("winput_message_loop")
                .encode_wide()
                .chain(iter::once(0))
                .collect::<Vec<_>>();

            let class = winuser::UnregisterClassW(class_name.as_ptr(), h_instance);
            if class != 0 {
                return; // Unregistration successful, exit the function
            }
        }

        // Unregistration failed, print the error and try again
        let error = std::format!(
            "INPUT CAPTURE | Failed to start: Os Error {}",
            WindowsError::from_last_error()
        );
        println!("UNREGISTER | Attempt {} failed: {}", attempts + 1, error);

        // Sleep for a short duration before the next attempt
        sleep(Duration::from_millis(1000));
        attempts += 1;
    }

    println!("UNREGISTER | All attempts failed");
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
    // The List of keys that will be blocked by the APP:
    let block_keys = [Vk::LeftWin, Vk::RightWin];

    loop {
        let button_channel = button_channel.clone();
        let mouse_channel = mouse_channel.clone();
        let shutdown_clone = shutdown.clone();

        match receiver.next_event() {
            message_loop::Event::Keyboard { vk, action, .. } if !block_keys.contains(&vk) => {
                let action_str = if action == Action::Press {
                    PRESS_KEYBOARD_ACTION
                } else if action == Action::Release {
                    RELEASE_KEYBOARD_ACTION
                } else {
                    continue;
                };

                let key = vk.into_u8().to_string();
                match handle_button_action(button_channel, action_str, key, shutdown_clone).await {
                    Ok(_) => (),
                    Err(e) => eprintln!("Failed to handle button action: {}", e),
                }
            }

            message_loop::Event::MouseButton { action, button } => {
                let action_str = if action == Action::Press {
                    PRESS_MOUSE_ACTION
                } else if action == Action::Release {
                    RELEASE_MOUSE_ACTION
                } else {
                    continue;
                };

                match handle_button_action(
                    button_channel,
                    action_str,
                    button_to_i32(button).to_string(),
                    shutdown_clone,
                )
                .await
                {
                    Ok(_) => (),
                    Err(e) => eprintln!("Failed to handle button action: {}", e),
                }
            }

            message_loop::Event::MouseWheel { delta, direction } => {
                if delta == 0.0 {
                    continue;
                }

                let action_str = if direction == WheelDirection::Horizontal {
                    SCROLL_HORIZONTAL_ACTION
                } else if direction == WheelDirection::Vertical {
                    SCROLL_VERTICAL_ACTION
                } else {
                    continue;
                };

                match handle_button_action(
                    button_channel,
                    action_str,
                    delta.to_string(),
                    shutdown_clone,
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

                    match mouse_channel_cpy
                        .send_text(std::format!("{} {}", x, y).as_str())
                        .await
                    {
                        Ok(_) => (),
                        Err(e) => eprintln!("Failed to send mouse event: {}", e),
                    }
                }
            }
            _ => (),
        }

        if shutdown.check_for_error().await {
            log::error!("INPUT CAPTURE | Shutdown received on check for error");
            receiver.clear();
            drop(receiver);
            break;
        };

        tokio::task::yield_now().await;
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
    if button_channel.ready_state()
        == webrtc::data_channel::data_channel_state::RTCDataChannelState::Open
    {
        if let Err(_e) = button_channel
            .send_text(std::format!("{}{}", action, text).as_str())
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
