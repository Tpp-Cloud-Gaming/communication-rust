use std::io::{Error, ErrorKind};
use std::sync::{mpsc, Arc};

use crate::gstreamer_pipeline::av_player::start_player;
use crate::input::input_capture::InputCapture;

use crate::utils::common_utils::wait_disconnect;
use crate::utils::error_tracker::ErrorTracker;
use crate::utils::shutdown;
use crate::utils::webrtc_const::{READ_TRACK_LIMIT, READ_TRACK_THRESHOLD};
use tokio::sync::Barrier;
use webrtc::api::media_engine::MIME_TYPE_H264;
use webrtc::data_channel::RTCDataChannel;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::{
    api::media_engine::MIME_TYPE_OPUS, rtp_transceiver::rtp_codec::RTPCodecType,
    track::track_remote::TrackRemote,
};

use crate::utils::latency_const::LATENCY_CHANNEL_LABEL;
use crate::utils::shutdown::Shutdown;
use crate::utils::webrtc_const::STUN_ADRESS;
use crate::webrtcommunication::communication::{encode, Communication};
use crate::webrtcommunication::latency::Latency;
use crate::websocketprotocol::socket_protocol::WsProtocol;

pub struct ReceiverSide {}

impl ReceiverSide {
    pub async fn init(client_name: &str, offerer_name: &str, game_name: &str) -> Result<(), Error> {
        // Initialize Log:
        let mut ws: WsProtocol = WsProtocol::ws_protocol().await?;
        ws.init_client(client_name, offerer_name, game_name).await?;

        let shutdown = Shutdown::new();

        let comunication = Communication::new(STUN_ADRESS.to_owned()).await?;

        let peer_connection = comunication.get_peer();

        let barrier = Arc::new(Barrier::new(4));
        let barrier_clone = barrier.clone();
        // Start mosue and keyboard capture
        let pc_cpy = peer_connection.clone();
        //TODO: Retornar errores ?
        let mut shutdown_cpya = shutdown.clone();
        let shutdown_cpy1 = shutdown.clone();

        tokio::spawn(async move {
            match InputCapture::new(pc_cpy, &mut shutdown_cpya).await {
                Ok(mut input_capture) => {
                    barrier_clone.wait().await;
                    match input_capture.start().await {
                        Ok(_) => {}
                        Err(e) => {
                            log::error!("Failed to start InputCapture: {}", e);
                            shutdown_cpy1
                                .notify_error(false, "Start input capture")
                                .await;
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to create InputCapture: {}", e);
                    shutdown_cpy1
                        .notify_error(false, "Create Input Capture")
                        .await;
                }
            }
        });

        // Create video frame channels
        let (tx_video, rx_video): (
            mpsc::Sender<(bool, Vec<u8>)>,
            mpsc::Receiver<(bool, Vec<u8>)>,
        ) = mpsc::channel();

        let (tx_audio, rx_audio): (
            mpsc::Sender<(bool, Vec<u8>)>,
            mpsc::Receiver<(bool, Vec<u8>)>,
        ) = mpsc::channel();

        let mut shutdown_audio = shutdown.clone();
        let barrier_clone_player = barrier.clone();
        tokio::spawn(async move {
            start_player(
                rx_video,
                rx_audio,
                &mut shutdown_audio,
                barrier_clone_player,
            )
            .await;
        });

        // Set a handler for when a new remote track starts, this handler saves buffers to disk as
        // an ivf file, since we could have multiple video tracks we provide a counter.
        // In your application this is where you would handle/process video
        set_on_track_handler(
            &peer_connection,
            tx_audio,
            tx_video,
            shutdown.clone(),
            barrier.clone(),
        );

        channel_handler(&peer_connection, shutdown.clone());

        // Allow us to receive 1 audio track
        if peer_connection
            .add_transceiver_from_kind(RTPCodecType::Audio, None)
            .await
            .is_err()
        {
            return Err(Error::new(
                ErrorKind::Other,
                "Error adding audio transceiver",
            ));
        }

        add_peer_connection_handler(&peer_connection, shutdown.clone());

        // Set the remote SessionDescription: ACA METER USER INPUT Y PEGAR EL SDP
        // Wait for the offer to be pasted

        let sdp = ws.wait_for_offerer_sdp().await?;
        comunication.set_sdp(sdp).await?;
        let peer_connection = comunication.get_peer();

        // Create an answer
        let answer = match peer_connection.create_answer(None).await {
            Ok(answer) => answer,
            Err(_) => return Err(Error::new(ErrorKind::Other, "Error creating answer")),
        };

        // Create channel that is blocked until ICE Gathering is complete
        let mut gather_complete = peer_connection.gathering_complete_promise().await;

        // Sets the LocalDescription, and starts our UDP listeners
        if peer_connection.set_local_description(answer).await.is_err() {
            return Err(Error::new(
                ErrorKind::Other,
                "Error setting local description",
            ));
        }

        // Block until ICE Gathering is complete, disabling trickle ICE
        // we do this because we only can exchange one signaling message
        // in a production application you should exchange ICE Candidates via OnICECandidate
        let _ = gather_complete.recv().await;

        // Output the answer in base64 so we can paste it in browser
        if let Some(local_desc) = peer_connection.local_description().await {
            // IMPRIMIR SDP EN BASE64
            let json_str = serde_json::to_string(&local_desc)?;
            let b64 = encode(&json_str);
            ws.send_sdp_to_offerer(offerer_name, &b64).await?;
            println!("{b64}");
        } else {
            log::error!("RECEIVER | Generate local_description failed!");
        }

        let mut wait_shutdown: bool = false;
        tokio::select! {
            _ = shutdown.wait_for_shutdown() => {
                log::info!("SENDER | Shutdown signal received");
                ws.force_stop_session(client_name).await?;
                wait_shutdown = true;
            }
            _ = wait_disconnect(shutdown.clone()) => {
                log::info!("SENDER | Disconnect signal received");
                ws.force_stop_session(client_name).await?;
            }
            _ = ws.wait_for_stop_session() => {
                log::info!("SENDER | Stop session signal received");
                shutdown.notify_error(true, "Stop session signal received").await;
            }
        }

        if !wait_shutdown {
            shutdown.wait_for_shutdown().await;
        }

        if peer_connection.close().await.is_err() {
            return Err(Error::new(
                ErrorKind::Other,
                "Error closing peer connection",
            ));
        }

        shutdown.shutdown();

        Ok(())
    }
}

/// Sets on track event for the provided connection
///
/// # Arguments
///
/// * `peer_connection` - A RTCPeerConnection.
/// * `tx_audio` - A channel to configure in case it is an audio track.
/// * `tx_audio` - A channel to configure in case it is a video track.
/// * `shutdown` -  Used for graceful shutdown.
fn set_on_track_handler(
    peer_connection: &Arc<RTCPeerConnection>,
    tx_audio: mpsc::Sender<(bool, Vec<u8>)>,
    tx_video: mpsc::Sender<(bool, Vec<u8>)>,
    shutdown: shutdown::Shutdown,
    barrier: Arc<Barrier>,
) {
    peer_connection.on_track(Box::new(move |track, _, _| {
        let codec = track.codec();
        let mime_type = codec.capability.mime_type.to_lowercase();
        let barrier_audio = barrier.clone();
        // Check if is a audio track
        if mime_type == MIME_TYPE_OPUS.to_lowercase() {
            let tx_audio_cpy = tx_audio.clone();
            let mut shutdown_cpy = shutdown.clone();
            return Box::pin(async move {
                tokio::spawn(async move {
                    barrier_audio.wait().await;
                    println!("RECEIVER | Got OPUS Track");
                    let _ = read_audio_track(track, tx_audio_cpy, &mut shutdown_cpy).await;
                });
            });
        };
        let barrier_video = barrier.clone();
        // Check if is a audio track
        if mime_type == MIME_TYPE_H264.to_lowercase() {
            let tx_video_cpy = tx_video.clone();
            let mut shutdown_cpy = shutdown.clone();
            return Box::pin(async move {
                tokio::spawn(async move {
                    barrier_video.wait().await;
                    println!("RECEIVER | Got H264 Track");
                    let _ = read_video_track(track, tx_video_cpy, &mut shutdown_cpy).await;
                });
            });
        };

        Box::pin(async {})
    }));
}

/// Reads RTP Packets on the provided audio track and sends them to the channel provided
///
/// # Arguments
///
/// * `track` - Audio track from which to read rtp packets
/// * `tx` - A channel to send the packets read
/// * `shutdown` -  Used for graceful shutdown.
///
/// # Return
/// Result containing `Ok(())` on success. Error on error.
async fn read_audio_track(
    track: Arc<TrackRemote>,
    tx: mpsc::Sender<(bool, Vec<u8>)>,
    shutdown: &mut shutdown::Shutdown,
) -> Result<(), Error> {
    let mut error_tracker = ErrorTracker::new(READ_TRACK_THRESHOLD, READ_TRACK_LIMIT);
    shutdown.add_task("Read audio track").await;

    loop {
        tokio::select! {
            result = track.read_rtp() => {
                if let Ok((rtp_packet, _)) = result {
                    let value = rtp_packet.payload.to_vec();
                    match tx.send((false,value)){
                        Ok(_) => {}
                        Err(e) => {
                            log::error!("RECEIVER | Error sending audio packet to channel: {e}");
                            //TODO: mejorar codigo repetido y unwrap
                            tx.send((true, vec![])).unwrap();
                            shutdown.notify_error(false, "Sending audio packet").await;
                            drop(tx);
                            return Err(Error::new(ErrorKind::Other, "Error sending audio packet to channel"));
                        }
                    }

                }else if error_tracker.increment_with_error(){
                        log::error!("RECEIVER | Max Attemps | Error reading RTP packet");
                        tx.send((true, vec![])).unwrap();
                        shutdown.notify_error(false,"Error sending rtp packet").await;
                        drop(tx);
                        return Err(Error::new(ErrorKind::Other, "Error reading RTP packet"));
                }else{
                        log::warn!("RECEIVER | Error reading RTP packet");
                };

            }
            _ = tokio::signal::ctrl_c() => {
                return Ok(());
            }
            _= shutdown.wait_for_error() => {
                log::info!("READ AUDIO TRACK | Shutdown received");
                tx.send((true, vec![])).unwrap();
                drop(tx);
                return Ok(());
            }
        }
    }
}

/// Reads data on the provided audio track and sends it to the channel provided
///
/// # Arguments
///
/// * `track` - Video track from which to read data
/// * `tx` - A channel to send the data read
/// * `shutdown` -  Used for graceful shutdown.
///
/// # Return
/// Result containing `Ok(())` on success. Error on error.
async fn read_video_track(
    track: Arc<TrackRemote>,
    tx: mpsc::Sender<(bool, Vec<u8>)>,
    shutdown: &mut shutdown::Shutdown,
) -> Result<(), Error> {
    let mut error_tracker = ErrorTracker::new(READ_TRACK_THRESHOLD, READ_TRACK_LIMIT);
    shutdown.add_task("Read video track").await;

    loop {
        let mut buff: [u8; 1400] = [0; 1400];
        tokio::select! {

            result = track.read(&mut buff) => {
                if let Ok((_rtp_packet, _)) = result {
                    match tx.send((false, buff.to_vec())){
                        Ok(_) => {}
                        Err(e) => {
                            log::error!("RECEIVER | Error sending video packet to channel: {e}");
                            shutdown.notify_error(false, "read video track sending").await;
                            //TODO: mejorar codigo repetido y unwrap
                            tx.send((true, vec![])).unwrap();
                            drop(tx);
                            return Err(Error::new(ErrorKind::Other, "Error sending video packet to channel"));
                        }

                    };

                }else if error_tracker.increment_with_error(){
                        log::error!("RECEIVER | Max Attemps | Error reading RTP packet");
                        shutdown.notify_error(false, "read video track max attemps").await;
                        tx.send((true, vec![])).unwrap();
                        drop(tx);
                        return Err(Error::new(ErrorKind::Other, "Error reading RTP packet"));
                }else{
                        log::warn!("RECEIVER | Error reading RTP packet");
                };

            }
            _ = tokio::signal::ctrl_c() => {
                return Ok(());
            }
            _= shutdown.wait_for_error() => {
                log::info!("READ VIDEO TRACK | Shutdown received");
                tx.send((true, vec![])).unwrap();
                drop(tx);
                return Ok(());
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
fn channel_handler(peer_connection: &Arc<RTCPeerConnection>, shutdown: shutdown::Shutdown) {
    // Register data channel creation handling
    peer_connection.on_data_channel(Box::new(move |d: Arc<RTCDataChannel>| {
        let d_label = d.label().to_owned();

        if d_label == LATENCY_CHANNEL_LABEL {
            let shutdown_cpy = shutdown.clone();
            Box::pin(async move {
                // Start the latency measurement
                if let Err(e) = Latency::start_latency_receiver(d).await {
                    log::error!("RECEIVER | Error starting latency receiver: {e}");
                    shutdown_cpy
                        .notify_error(false, "Error sending latency")
                        .await;
                }
            })
        } else {
            Box::pin(async move {
                log::info!("RECEIVER |New DataChannel has been opened | {d_label}");
            })
        }
    }));
}
fn add_peer_connection_handler(
    peer_connection: &Arc<RTCPeerConnection>,
    shutdown: shutdown::Shutdown,
) {
    peer_connection.on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
        log::info!("Peer Connection State has changed {s}");

        if s == RTCPeerConnectionState::Connected {
            log::info!("Peer Connection state: Connected");
            return Box::pin(async move {
                println!("RECEIVER | Barrier waiting");
                println!("RECEIVER | Barrier released");
            });
        }

        if s == RTCPeerConnectionState::Closed {
            log::error!("RECEIVER | Peer connection state: Closed");
            let shutdown_cpy = shutdown.clone();
            return Box::pin(async move {
                shutdown_cpy
                    .notify_error(true, "Peer connection closed")
                    .await;
                log::error!("RECEIVER | Notify error sended");
            });
        }

        if s == RTCPeerConnectionState::Failed {
            // Wait until PeerConnection has had no network activity for 30 seconds or another failure. It may be reconnected using an ICE Restart.
            // Use webrtc.PeerConnectionStateDisconnected if you are interested in detecting faster timeout.
            // Note that the PeerConnection may come back from PeerConnectionStateDisconnected.
            log::error!("SENDER | Peer connection state: Failed");
            let shutdown_cpy = shutdown.clone();
            return Box::pin(async move {
                shutdown_cpy
                    .notify_error(true, "Peer connection failed")
                    .await;
                log::error!("RECEIVER | Notify error sended");
            });
        }

        if s == RTCPeerConnectionState::Disconnected {
            log::error!("RECEIVER | Peer connection state: Disconnected");
            let shutdown_cpy = shutdown.clone();
            return Box::pin(async move {
                shutdown_cpy
                    .notify_error(true, "Peer connection disconnected")
                    .await;
                log::error!("RECEIVER | Notify error sended");
            });
        }

        Box::pin(async {})
    }));
}
