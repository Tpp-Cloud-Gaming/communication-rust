use gstreamer::glib::JoinHandle;
use std::io::{Error, ErrorKind};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::channel;
use tokio::sync::mpsc::Receiver;
use tokio::sync::Barrier;

use crate::front_connection::front_protocol::FrontConnection;
use crate::gstreamer_pipeline::av_capture::start_capture;
use crate::services::sender_utils::{get_handler, initialize_game};
use crate::utils::common_utils::wait_disconnect;
use crate::utils::shutdown::Shutdown;
use crate::webrtcommunication::communication::{encode, Communication};

use crate::input::input_const::{KEYBOARD_CHANNEL_LABEL, MOUSE_CHANNEL_LABEL};
use crate::output::button_controller::ButtonController;
use crate::output::mouse_controller::MouseController;
use webrtc::data_channel::RTCDataChannel;

use crate::utils::shutdown;
use webrtc::api::media_engine::{MIME_TYPE_H264, MIME_TYPE_OPUS};
use webrtc::media::Sample;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::rtp_transceiver::rtp_sender::RTCRtpSender;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::track::track_local::{TrackLocal, TrackLocalWriter};

//use std::process::Command;
use winapi::um::processthreadsapi::OpenProcess;
use winapi::um::processthreadsapi::TerminateProcess;
use winapi::um::winnt::PROCESS_TERMINATE;

use crate::utils::webrtc_const::{
    AUDIO_CHANNELS, AUDIO_SAMPLE_RATE, AUDIO_TRACK_ID, SEND_TRACK_LIMIT, SEND_TRACK_THRESHOLD,
    STREAM_TRACK_ID, STUN_ADRESS, VIDEO_TRACK_ID,
};
use crate::webrtcommunication::latency::Latency;
use crate::websocketprotocol::socket_protocol::{ClientInfo, WsProtocol};

pub struct SenderSide {}
impl SenderSide {
    pub async fn init(offerer_name: &str, ws: &mut WsProtocol) -> Result<(), Error> {
        let shutdown = Shutdown::new();

        // Wait for client to request a connection
        ws.init_offer(offerer_name).await?;
        let mut client_info: Option<ClientInfo> = None;

        tokio::select! {
            cf = ws.wait_for_game_solicitude() => {
                client_info = Some(cf?);
            }
            f = FrontConnection::new("3132") => {
                f.unwrap().waiting_to_disconnect().await;
                return Ok(());
            }
        }

        log::info!("SENDER | Received client info | {:?}", client_info);

        let new_client = match client_info {
            Some(c) => c,
            None => {
                return Err(Error::new(ErrorKind::Other, "Error receiving client info"));
            }
        };
        // Start game
        let game_path = &new_client.game_path;

        initialize_game(game_path)?;

        let barrier = Arc::new(Barrier::new(5));

        //Create audio frames channels
        let (tx_audio, rx_audio) = channel(100);

        // Create video frame channels
        let (tx_video, rx_video) = channel(100);

        let comunication =
            check_error(Communication::new(STUN_ADRESS.to_owned()).await, &shutdown).await?;

        let (hwnd, pid) = match get_handler(game_path) {
            Ok((hwnd, pid)) => (hwnd, pid),
            Err(_) => {
                shutdown.notify_error(true, "get_handler").await;
                return Err(Error::new(ErrorKind::Other, "Error getting handler"));
            }
        };

        // Start the video capture
        let mut shutdown_capture = shutdown.clone();

        let barrier_video = barrier.clone();

        tokio::spawn(async move {
            start_capture(
                tx_video,
                tx_audio,
                &mut shutdown_capture,
                barrier_video,
                hwnd,
            )
            .await;
        });

        let pc = comunication.get_peer();

        let (_rtp_sender, audio_track) =
            create_track_sample(pc.clone(), shutdown.clone(), MIME_TYPE_OPUS, AUDIO_TRACK_ID)
                .await?;
        let (rtp_video_sender, video_track) =
            create_track_rtp(pc.clone(), shutdown.clone(), MIME_TYPE_H264, VIDEO_TRACK_ID).await?;

        check_error(Latency::start_latency_sender(pc.clone()).await, &shutdown).await?;

        channel_handler(&pc, shutdown.clone());

        let shutdown_cpy_3 = shutdown.clone();
        tokio::spawn(async move {
            read_rtcp(&mut shutdown_cpy_3.clone(), rtp_video_sender).await;
        });

        let barrier_audio_send = barrier.clone();
        let mut shutdown_cpy_2 = shutdown.clone();
        tokio::spawn(async move {
            start_audio_sending(
                barrier_audio_send,
                rx_audio,
                audio_track,
                &mut shutdown_cpy_2,
            )
            .await;
        });

        let barrier_video_send = barrier.clone();
        let mut shutdown_cpy_4 = shutdown.clone();
        tokio::spawn(async move {
            start_video_sending(
                barrier_video_send,
                rx_video,
                video_track,
                &mut shutdown_cpy_4,
            )
            .await;
        });

        set_peer_events(&pc, barrier.clone(), shutdown.clone());

        // Create an answer to send to the other process
        let offer = match pc.create_offer(None).await {
            Ok(offer) => offer,
            Err(_) => {
                shutdown.notify_error(true, "Create offer").await;
                return Err(Error::new(ErrorKind::Other, "Error creating offer"));
            }
        };
        // Create channel that is blocked until ICE Gathering is complete
        let mut gather_complete = pc.gathering_complete_promise().await;

        // Sets the LocalDescription, and starts our UDP listeners
        if let Err(_e) = pc.set_local_description(offer).await {
            shutdown.notify_error(true, "Set local description").await;
            return Err(Error::new(
                ErrorKind::Other,
                "Error setting local description",
            ));
        }

        let _ = gather_complete.recv().await;

        if let Some(local_desc) = pc.local_description().await {
            let json_str = serde_json::to_string(&local_desc)?;
            let b64 = encode(&json_str);
            ws.send_sdp_to_client(&new_client.client_name, &b64).await?;
            println!("{b64}");
        } else {
            log::error!("SENDER | Generate local_description failed");
            shutdown.notify_error(true, "Local description").await;
            return Err(Error::new(
                ErrorKind::Other,
                "Generate local_description failed",
            ));
        }

        let client_sdp = ws.wait_for_client_sdp().await?;
        check_error(comunication.set_sdp(client_sdp).await, &shutdown).await?;

        let mut barrier_passed: bool = false;
        tokio::select! {
            _ = barrier.wait() => {
                barrier_passed = true
            }
            _ = shutdown.wait_for_shutdown() => {
                log::error!("RECEIVER | Error notifier signal");
            }
        };

        if barrier_passed {
            ws.start_session(offerer_name, new_client.client_name.as_str())
                .await?;
            println!("SENDER | Start session msg sended");

            let mut wait_shutdown: bool = false;
            tokio::select! {
                _ = shutdown.wait_for_shutdown() => {
                    log::info!("SENDER | Shutdown signal received");
                    ws.force_stop_session(offerer_name).await?;
                    wait_shutdown = true;
                }
                _  = wait_disconnect(shutdown.clone()) => {
                    log::info!("SENDER | Disconnect signal received");
                    ws.force_stop_session(offerer_name).await?;
                }
                _ = ws.wait_for_stop_session() => {
                    log::info!("SENDER | Stop session signal received");
                    shutdown.notify_error(true, "Stop session signal received").await;
                }
            }

            if !wait_shutdown {
                shutdown.wait_for_shutdown().await;
            }
        }

        kill_process(pid)?;

        if pc.close().await.is_err() {
            return Err(Error::new(
                ErrorKind::Other,
                "Error closing peer connection",
            ));
        }

        Ok(())
    }
}

/// Creates a TrackLocalStaticSample and adds it to the provided peer connection
///
/// # Arguments
///
/// * `pc` - A RTCPeerConnection to add the track.
/// * `shutdown` - Used for graceful shutdown.
/// * `mime_type` - The mime type for the configuration of the track.
/// * `track_id` - The id provided for the configuration of the track.
///
/// # Return
/// Result containing `Ok((Arc<RTCRtpSender>, Arc<TrackLocalStaticSample>))` on success. Error on error.
async fn create_track_sample(
    pc: Arc<RTCPeerConnection>,
    shutdown: shutdown::Shutdown,
    mime_type: &str,
    track_id: &str,
) -> Result<(Arc<RTCRtpSender>, Arc<TrackLocalStaticSample>), Error> {
    let track = Arc::new(TrackLocalStaticSample::new(
        RTCRtpCodecCapability {
            mime_type: mime_type.to_owned(),
            ..Default::default()
        },
        track_id.to_owned(),
        STREAM_TRACK_ID.to_owned(),
    ));
    match pc
        .add_track(Arc::clone(&track) as Arc<dyn TrackLocal + Send + Sync>)
        .await
    {
        Ok(rtp_sender) => Ok((rtp_sender, track)),
        Err(_) => {
            shutdown.notify_error(true, "Add track sample").await;
            Err(Error::new(
                ErrorKind::Other,
                "Error setting local description",
            ))
        }
    }
}

/// Creates a TrackLocalStaticRTP and adds it to the provided peer connection
///
/// # Arguments
///
/// * `pc` - A RTCPeerConnection to add the track.
/// * `shutdown` - Used for graceful shutdown.
/// * `mime_type` - The mime type for the configuration of the track.
/// * `track_id` - The id provided for the configuration of the track.
///
/// # Return
/// Result containing `Ok((Arc<RTCRtpSender>, Arc<TrackLocalStaticRTP>))` on success. Error on error.
async fn create_track_rtp(
    pc: Arc<RTCPeerConnection>,
    shutdown: shutdown::Shutdown,
    mime_type: &str,
    track_id: &str,
) -> Result<(Arc<RTCRtpSender>, Arc<TrackLocalStaticRTP>), Error> {
    let track = Arc::new(TrackLocalStaticRTP::new(
        RTCRtpCodecCapability {
            mime_type: mime_type.to_owned(),
            ..Default::default()
        },
        track_id.to_owned(),
        STREAM_TRACK_ID.to_owned(),
    ));
    match pc
        .add_track(Arc::clone(&track) as Arc<dyn TrackLocal + Send + Sync>)
        .await
    {
        Ok(rtp_sender) => Ok((rtp_sender, track)),
        Err(_) => {
            shutdown.notify_error(true, "Add track rtp").await;
            Err(Error::new(
                ErrorKind::Other,
                "Error setting local description",
            ))
        }
    }
}

/// Sets the event handlers for ice connection/peer connection state change on the provided connection
///
/// # Arguments
///
/// * `pc` - A RTCPeerConnection.
/// * `done_tx` - A channel to send the message if the peer connection fails.
/// * `barrier` - Used for synchronization.
fn set_peer_events(
    pc: &Arc<RTCPeerConnection>,
    barrier: Arc<Barrier>,
    shutdown: shutdown::Shutdown,
) {
    // Set the handler for ICE connection state
    // This will notify you when the peer has connected/disconnected
    // pc.on_ice_connection_state_change(Box::new(move |connection_state: RTCIceConnectionState| {
    //     log::info!("SENDER | ICE Connection State has changed | {connection_state}");
    //     if connection_state == RTCIceConnectionState::Connected {}
    //     Box::pin(async {})
    // }));

    // Set the handler for Peer connection state
    // This will notify you when the peer has connected/disconnected

    pc.on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
        log::info!("Peer Connection State has changed {s}");

        if s == RTCPeerConnectionState::Connected {
            log::info!("Peer Connection state: Connected");
            let barrier_cpy = barrier.clone();
            return Box::pin(async move {
                println!("SENDER | Barrier waiting");
                barrier_cpy.wait().await;
                println!("SENDER | Barrier released");
            });
        }

        if s == RTCPeerConnectionState::Closed {
            log::error!("SENDER | Peer connection state: Closed");
            let shutdown_cpy = shutdown.clone();
            return Box::pin(async move {
                shutdown_cpy
                    .notify_error(true, "Peer connection closed")
                    .await;
                log::error!("SENDER | Notify error sended");
            });
        }

        if s == RTCPeerConnectionState::Failed {
            log::error!("SENDER | Peer connection state: Failed");
            let shutdown_cpy = shutdown.clone();
            return Box::pin(async move {
                shutdown_cpy
                    .notify_error(true, "Peer connection Failed")
                    .await;
                log::error!("SENDER | Notify error sended");
            });
        }

        if s == RTCPeerConnectionState::Disconnected {
            log::error!("SENDER | Peer connection state: Disconnected");
            let shutdown_cpy = shutdown.clone();
            //let _ = done_tx.try_send(());
            return Box::pin(async move {
                shutdown_cpy
                    .notify_error(true, "Peer connection disconnected")
                    .await;
                log::error!("SENDER | Notify error sended");
            });
        }

        Box::pin(async {})
    }));
}

/// Checks the result provided and notifies the shutdown in case of error
///
/// # Arguments
///
/// * `result` - The result to check.
/// * `shutdown` -  Used for graceful shutdown.
///
/// # Return
/// The result provided as argument
async fn check_error<T, E>(result: Result<T, E>, shutdown: &Shutdown) -> Result<T, E> {
    if result.is_err() {
        shutdown.notify_error(true, "Check error").await;
    }
    result
}

/// Reads incoming rtcp packets
///
/// # Arguments
///
/// * `shutdown` -  Used for graceful shutdown.
/// * `rtp_sender` -  RTCRtpSender from which to read messages.
async fn read_rtcp(shutdown: &mut shutdown::Shutdown, rtp_sender: Arc<RTCRtpSender>) {
    shutdown.add_task("Read rtcp").await;
    let mut rtcp_buf = vec![0u8; 1500];
    loop {
        tokio::select! {
            _ = rtp_sender.read(&mut rtcp_buf) => {

            }
            _ = shutdown.wait_for_error() => {
                log::error!("SENDER | read_rtcp | Shutdown signal received");
                break;
            }
        }
    }
}

/// Receives audio samples and sends them
///
/// # Arguments
///
/// * `barrier_audio_send` - Used for synchronization.
/// * `rx` - A channel to receive samples.
/// * `audio_track` - Track to write the samples to.
/// * `shutdown` -  Used for graceful shutdown.
async fn start_audio_sending(
    barrier_audio_send: Arc<Barrier>,
    mut rx: Receiver<Vec<u8>>,
    audio_track: Arc<TrackLocalStaticSample>,
    shutdown: &mut shutdown::Shutdown,
) {
    shutdown.add_task("Audio sending").await;

    tokio::select! {
        _ = shutdown.wait_for_error() => {
            log::error!("SENDER | START AUDIO SENDING | Shutdown signal received");
            return;
        },
        _ = barrier_audio_send.wait() => {
            log::info!("SENDER | START AUDIO SENDING | Barrier passed");
        }
    }

    let mut error_tracker_write =
        crate::utils::error_tracker::ErrorTracker::new(SEND_TRACK_THRESHOLD, SEND_TRACK_LIMIT);

    let sample_duration =
        Duration::from_millis((AUDIO_CHANNELS as u64 * 10000000) / AUDIO_SAMPLE_RATE as u64); //TODO: no hardcodear

    let mut data = match rx.recv().await {
        Some(d) => d,
        None => {
            shutdown.notify_error(false, "No audio data received").await;
            return;
        }
    };

    loop {
        if let Err(err) = audio_track
            .write_sample(&Sample {
                data: data.clone().into(),
                duration: sample_duration,
                ..Default::default()
            })
            .await
        {
            log::warn!("SENDER | Error writing sample | {}", err);
            if error_tracker_write.increment_with_error() {
                log::error!("SENDER | Max attemps | Error writing sample | {}", err);
                shutdown.notify_error(false, "Error writing sample").await;
                return;
            } else {
                log::warn!("SENDER | Error writing sample | {}", err);
            };
            continue;
        } else {
            error_tracker_write.increment();
        }

        tokio::select! {
            a = rx.recv() => {
                match a {
                    Some(d) => {
                        data = d;
                    }
                    None => {
                        log::error!("SENDER | Error receiving audio data |");
                        shutdown
                            .notify_error(false, "Error receiving audio data")
                            .await;
                        return;
                    }
                }
            },
            _ = shutdown.wait_for_error() => {
                log::error!("SENDER | START AUDIO SENDING | Shutdown signal received");
                break;
            }
        }
    }
}

/// Receives video samples and sends them
///
/// # Arguments
///
/// * `barrier_video_send` - Used for synchronization.
/// * `rx` - A channel to receive samples.
/// * `video_track` - Track to write the samples to.
/// * `shutdown` -  Used for graceful shutdown.
async fn start_video_sending(
    barrier_video_send: Arc<Barrier>,
    mut rx: Receiver<Vec<u8>>,
    video_track: Arc<TrackLocalStaticRTP>,
    shutdown: &mut shutdown::Shutdown,
) {
    shutdown.add_task("Video sending").await;

    tokio::select! {
        _ = shutdown.wait_for_error() => {
            log::error!("SENDER | START VIDEO SENDING | Shutdown signal received");
            return;
        },
        _ =  barrier_video_send.wait() => {
            log::info!("SENDER | START VIDEO SENDING | Barrier passed");
        }
    }

    let mut error_tracker_write =
        crate::utils::error_tracker::ErrorTracker::new(SEND_TRACK_THRESHOLD, SEND_TRACK_LIMIT);

    let mut data = match rx.recv().await {
        Some(d) => d,
        None => {
            shutdown.notify_error(false, "No video data received").await;
            return;
        }
    };

    loop {
        if let Err(err) = video_track.write(&data).await {
            log::warn!("SENDER | Error writing sample | {}", err);
            if error_tracker_write.increment_with_error() {
                log::error!("SENDER | Max attemps | Error writing sample | {}", err);
                shutdown.notify_error(false, "Error writing sample").await;
                return;
            } else {
                log::warn!("SENDER | Error writing sample | {}", err);
            };
            continue;
        } else {
            error_tracker_write.increment();
        }

        tokio::select! {
            a = rx.recv() => {
                match a {
                    Some(d) => {
                        data = d;
                    }
                    None => {
                        log::error!("SENDER | Error receiving video data |");
                        shutdown
                            .notify_error(false, "Error receiving video data")
                            .await;
                        return;
                    }
                }
            },
            _ = shutdown.wait_for_error() => {
                log::error!("SENDER | START VIDEO SENDING | Shutdown signal received");
                break;
            }
        }
    }
}

/// Sets on data channel event for the given connection
///
/// # Arguments
///
/// * `peer_conection` - A RTCPeerConnection
/// * `shutdown` -  Used for graceful shutdown.
fn channel_handler(peer_connection: &Arc<RTCPeerConnection>, _shutdown: shutdown::Shutdown) {
    // Register data channel creation handling
    peer_connection.on_data_channel(Box::new(move |d: Arc<RTCDataChannel>| {
        let d_label = d.label().to_owned();

        if d_label == MOUSE_CHANNEL_LABEL {
            Box::pin(async {
                MouseController::start_mouse_controller(d);
            })
        } else if d_label == KEYBOARD_CHANNEL_LABEL {
            Box::pin(async {
                ButtonController::start_keyboard_controller(d);
            })
        } else {
            Box::pin(async move {
                log::info!("RECEIVER |New DataChannel has been opened | {d_label}");
            })
        }
    }));
}

fn kill_process(pid: u32) -> std::io::Result<()> {
    unsafe {
        let h_process = OpenProcess(PROCESS_TERMINATE, 0, pid);
        if h_process.is_null() {
            println!("Failed to open the process.");
            return Ok(());
        }

        TerminateProcess(h_process, 1);
    };

    Ok(())
}
