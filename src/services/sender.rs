use std::io::{Error, ErrorKind};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::channel;
use tokio::sync::mpsc::Receiver;
use tokio::sync::Barrier;

use crate::gstreamer_pipeline::av_capture::start_capture;
use crate::services::sender_utils::{initialize_game, select_game_window};
use crate::utils::shutdown::Shutdown;
use crate::webrtcommunication::communication::{encode, Communication};

use crate::input::input_const::{KEYBOARD_CHANNEL_LABEL, MOUSE_CHANNEL_LABEL};
use crate::output::button_controller::ButtonController;
use crate::output::mouse_controller::MouseController;
use webrtc::data_channel::RTCDataChannel;

use crate::utils::shutdown;
use webrtc::api::media_engine::{MIME_TYPE_H264, MIME_TYPE_OPUS};
use webrtc::ice_transport::ice_connection_state::RTCIceConnectionState;
use webrtc::media::Sample;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::rtp_transceiver::rtp_sender::RTCRtpSender;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::track::track_local::{TrackLocal, TrackLocalWriter};

use crate::utils::webrtc_const::{
    AUDIO_CHANNELS, AUDIO_SAMPLE_RATE, AUDIO_TRACK_ID, SEND_TRACK_LIMIT, SEND_TRACK_THRESHOLD,
    STREAM_TRACK_ID, STUN_ADRESS, VIDEO_TRACK_ID,
};
use crate::webrtcommunication::latency::Latency;
use crate::websocketprotocol::websocketprotocol::WsProtocol;

pub struct SenderSide {}
impl SenderSide {
    pub async fn new(offerer_name: &str) -> Result<(), Error> {
        //Start log
        env_logger::builder().format_target(false).init();
        // Start shutdown
        let shutdown = Shutdown::new();

        // WAit for client to request a connection
        let mut ws = WsProtocol::ws_protocol().await?;
        ws.init_offer(offerer_name).await?;
        let client_info = ws.wait_for_game_solicitude().await?;

        log::info!("SENDER | Received client info | {:?}", client_info);

        // Start game
        let game_path = &client_info.game_path;
        
        let _game_id = initialize_game(game_path)?;

        let barrier = Arc::new(Barrier::new(4));

        //Create audio frames channels
        let (tx_audio, rx_audio) = channel(100);

        // Create video frame channels
        let (tx_video, rx_video) = channel(100);

        let comunication =
            check_error(Communication::new(STUN_ADRESS.to_owned()).await, &shutdown).await?;


        // Get window id of the game
        let hwnd = select_game_window(game_path);

        // Start the video capture
        let mut shutdown_capture = shutdown.clone();

        let barrier_video = barrier.clone();
        tokio::spawn(async move {
            start_capture(tx_video, tx_audio,&mut shutdown_capture, barrier_video, hwnd).await;
        });

        let (done_tx, mut done_rx) = tokio::sync::mpsc::channel::<()>(1);

        let pc = comunication.get_peer();

        let (_rtp_sender, audio_track) =
            create_track_sample(pc.clone(), shutdown.clone(), MIME_TYPE_OPUS, AUDIO_TRACK_ID)
                .await?;
        let (rtp_video_sender, video_track) =
            create_track_rtp(pc.clone(), shutdown.clone(), MIME_TYPE_H264, VIDEO_TRACK_ID).await?;

        // Start the latency measurement
        check_error(Latency::start_latency_sender(pc.clone()).await, &shutdown).await?;

        channel_handler(&pc, shutdown.clone());

        let shutdown_cpy_3 = shutdown.clone();
        tokio::spawn(async move {
            read_rtcp(&mut shutdown_cpy_3.clone(), rtp_video_sender).await;
        });

        let barrier_audio_send = barrier.clone();
        let mut shutdown_cpy_2 = shutdown.clone();
        tokio::spawn(async move {
            start_audio_sending(barrier_audio_send, rx_audio, audio_track, &mut  shutdown_cpy_2).await;
        });

        let barrier_video_send = barrier.clone();
        let mut shutdown_cpy_4 = shutdown.clone();
        tokio::spawn(async move {
            start_video_sending(barrier_video_send, rx_video, video_track,  &mut shutdown_cpy_4).await;
        });

        set_peer_events(&pc, done_tx, barrier.clone(), shutdown.clone());

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
            ws.send_sdp_to_client(&client_info.client_name, &b64)
                .await?;
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

        println!("Press ctrl-c to stop");
        tokio::select! {
            _ = done_rx.recv() => {
                log::info!("SENDER | Received done signal");
            }
            _ = tokio::signal::ctrl_c() => {
                println!();
            }
            _ = shutdown.wait_for_shutdown() => {
                log::error!("RECEIVER | Error notifier signal");
                return Ok(())
            }
        };

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
    done_tx: tokio::sync::mpsc::Sender<()>,
    barrier: Arc<Barrier>,
    shutdown: shutdown::Shutdown
) {
    // Set the handler for ICE connection state
    // This will notify you when the peer has connected/disconnected
    pc.on_ice_connection_state_change(Box::new(move |connection_state: RTCIceConnectionState| {
        log::info!("SENDER | ICE Connection State has changed | {connection_state}");
        if connection_state == RTCIceConnectionState::Connected {
        }
        Box::pin(async {})
    }));

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
                shutdown_cpy.notify_error(true, "Peer connection closed").await;
                log::error!("SENDER | Notify error sended");
                
            });

        }

        
        if s == RTCPeerConnectionState::Failed {
            // Wait until PeerConnection has had no network activity for 30 seconds or another failure. It may be reconnected using an ICE Restart.
            // Use webrtc.PeerConnectionStateDisconnected if you are interested in detecting faster timeout.
            // Note that the PeerConnection may come back from PeerConnectionStateDisconnected.
            log::error!("SENDER | Peer connection state: Failed");
            let _ = done_tx.try_send(());
        }
        
        if s == RTCPeerConnectionState::Disconnected {
            log::error!("SENDER | Peer connection state: Disconnected");
            let shutdown_cpy = shutdown.clone();
            return Box::pin(async move {    
                shutdown_cpy.notify_error(true, "Peer connection disconnected").await;
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
                log::info!("SENDER | Shutdown signal received");
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
    // Wait for other tasks
    barrier_audio_send.wait().await;
    
    let mut error_tracker =
    crate::utils::error_tracker::ErrorTracker::new(SEND_TRACK_THRESHOLD, SEND_TRACK_LIMIT);
    
    let sample_duration =
        Duration::from_millis((AUDIO_CHANNELS as u64 * 10000000) / AUDIO_SAMPLE_RATE as u64); //TODO: no hardcodear

        let mut data = match rx.recv().await {
            Some(d) => {
                error_tracker.increment();
                d
            }
            None => {
                    shutdown.notify_error(false,"No audio data received" ).await;
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
            if error_tracker.increment_with_error() {
                log::error!("SENDER | Max attemps | Error writing sample | {}", err);
                shutdown.notify_error(false, "Error writing sample").await;
                return;
            } else {
                log::warn!("SENDER | Error writing sample | {}", err);
            };
            continue;
        } else {
            error_tracker.increment();
        }


        data = match rx.try_recv() {
            Ok(d) => {
                error_tracker.increment();
                d
            }
            Err(_) => {
                if error_tracker.increment_with_error() {
                    log::error!("SENDER | Max attemps | Error receiving audio data | ",);
                    shutdown.notify_error(false, "Error receiveing audio data").await;
                    return;
                } else {
                    log::warn!("SENDER | Error receiving audio data | ");
                };
                continue;
            }
        };

        if shutdown.check_for_error().await {
            return;
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
    // Wait for connection established
    // TODO: Esto puede generar delay me parece
    barrier_video_send.wait().await;

    let mut error_tracker =
        crate::utils::error_tracker::ErrorTracker::new(SEND_TRACK_THRESHOLD, SEND_TRACK_LIMIT);

        let mut data = match rx.recv().await {
            Some(d) => {
                error_tracker.increment();
                d
            }
            None => {
                    shutdown.notify_error(false,"No video data received" ).await;
                    return;
            }
        };

    loop {
        if let Err(err) = video_track.write(&data).await {
            log::warn!("SENDER | Error writing sample | {}", err);
            if error_tracker.increment_with_error() {
                log::error!("SENDER | Max attemps | Error writing sample | {}", err);
                shutdown.notify_error(false, "Error writing sample").await;
                return;
            } else {
                log::warn!("SENDER | Error writing sample | {}", err);
            };
            continue;
        } else {
            error_tracker.increment();
        }


        data = match rx.try_recv() {
            Ok(d) => {
                error_tracker.increment();
                d
            }
            Err(_) => {
                if error_tracker.increment_with_error() {
                    //log::error!("SENDER | Max attemps | Error receiving video data |",);
                    //shutdown.notify_error(false,"Error max attemps on video sending" ).await;
                    return;
                } else {
                    log::info!("SENDER | Error receiving video data |");
                };
                continue;
            }
        };

        //let sample_duration =
        //    Duration::from_millis(1000 / 30 as u64); //TODO: no hardcodear
        
        if shutdown.check_for_error().await {
            return;
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
