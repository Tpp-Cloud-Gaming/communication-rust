pub mod audio;
pub mod input;
pub mod utils;
pub mod output;
pub mod webrtcommunication;
use std::io::{Error, ErrorKind};
use std::sync::Arc;

use input::input_const::{KEYBOARD_CHANNEL_LABEL, MOUSE_CHANNEL_LABEL};
use utils::error_tracker::ErrorTracker;
use utils::shutdown;
use utils::webrtc_const::{READ_TRACK_LIMIT, READ_TRACK_THRESHOLD};
use webrtc::data_channel::RTCDataChannel;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::{
    api::media_engine::MIME_TYPE_OPUS, rtp_transceiver::rtp_codec::RTPCodecType,
    track::track_remote::TrackRemote,
};

use std::sync::Mutex;
use tokio::sync::mpsc::{Receiver, Sender};

use cpal::traits::StreamTrait;

use crate::audio::audio_decoder::AudioDecoder;
use crate::utils::common_utils::get_args;
use crate::utils::latency_const::LATENCY_CHANNEL_LABEL;
use crate::utils::shutdown::Shutdown;
use crate::utils::webrtc_const::{ENCODE_BUFFER_SIZE, STUN_ADRESS};
use crate::webrtcommunication::communication::{encode, Communication};
use crate::webrtcommunication::latency::Latency;
use crate::output::output_controller::MouseController;


#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize Log:
    env_logger::builder().format_target(false).init();
    let shutdown = Shutdown::new();

    //Check for CLI args
    let audio_device = get_args();

    let (tx_decoder_1, rx_decoder_1): (Sender<f32>, Receiver<f32>) =
        tokio::sync::mpsc::channel(ENCODE_BUFFER_SIZE);
    let audio_player = match audio::audio_player::AudioPlayer::new(
        audio_device,
        Arc::new(Mutex::new(rx_decoder_1)),
    ) {
        Ok(audio_player) => audio_player,
        Err(e) => {
            log::error!("RECEIVER | Error creating audio player: {e}");
            return Err(Error::new(ErrorKind::Other, "Error creating audio player"));
        }
    };
    let stream = match audio_player.start() {
        Ok(stream) => stream,
        Err(e) => {
            log::error!("RECEIVER | Error starting audio player: {e}");
            return Err(Error::new(ErrorKind::Other, "Error starting audio player"));
        }
    };
    if let Err(e) = stream.play() {
        log::error!("RECEIVER | Error playing audio player: {e}");
        return Err(Error::new(ErrorKind::Other, "Error playing audio player"));
    };

    let comunication = Communication::new(STUN_ADRESS.to_owned()).await?;

    let peer_connection = comunication.get_peer();

    // Set a handler for when a new remote track starts, this handler saves buffers to disk as
    // an ivf file, since we could have multiple video tracks we provide a counter.
    // In your application this is where you would handle/process video
    set_on_track_handler(&peer_connection, tx_decoder_1, shutdown.clone());

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

    //set_on_ice_connection_state_change_handler(&peer_connection, shutdown.clone());

    // Set the remote SessionDescription: ACA METER USER INPUT Y PEGAR EL SDP
    // Wait for the offer to be pasted
    comunication.set_sdp().await?;
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
        println!("{b64}");
    } else {
        log::error!("RECEIVER | Generate local_description failed!");
    }

    println!("Press ctrl-c to stop");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            log::info!("RECEIVER | ctrl-c signal");
            println!();
        }
        _ = shutdown.wait_for_shutdown() => {
            log::info!("RECEIVER | Error notifier signal");
        }
    };

    if peer_connection.close().await.is_err() {
        return Err(Error::new(
            ErrorKind::Other,
            "Error closing peer connection",
        ));
    }

    shutdown.shutdown();

    Ok(())
}

fn set_on_track_handler(
    peer_connection: &Arc<RTCPeerConnection>,
    tx_decoder_1: Sender<f32>,
    shutdown: shutdown::Shutdown,
) {
    peer_connection.on_track(Box::new(move |track, _, _| {
        let codec = track.codec();
        let mime_type = codec.capability.mime_type.to_lowercase();

        // Check if is a audio track
        if mime_type == MIME_TYPE_OPUS.to_lowercase() {
            let tx_decoder_cpy = tx_decoder_1.clone();
            let shutdown_cpy = shutdown.clone();
            return Box::pin(async move {
                log::info!("RECEIVER | Got OPUS Track");
                tokio::spawn(async move {
                    let _ = read_track(track, &tx_decoder_cpy, shutdown_cpy).await;
                });
            });
        };

        Box::pin(async {})
    }));
}

//Esta funcion solo sirve para que detecte si algun on ice pasa a connection state failed y ahi
// mande un signal para que todo termine

// Set the handler for ICE connection state
// This will notify you when the peer has connected/disconnected
// fn set_on_ice_connection_state_change_handler(
//     peer_connection: &Arc<RTCPeerConnection>,
//     _shutdown: shutdown::Shutdown,
// ) {
//     peer_connection.on_ice_connection_state_change(Box::new(
//         move |connection_state: RTCIceConnectionState| {
//             log::info!("RECEIVER | ICE Connection State has changed | {connection_state}");

//             // if connection_state == RTCIceConnectionState::Connected {
//             //     //let shutdown_cpy = shutdown.clone();
//             // } else if connection_state == RTCIceConnectionState::Failed {
//             //     TODO: ver que hacer en este escenario
//             //     let shutdown_cpy = shutdown.clone();
//             //     _ = Box::pin(async move {
//             //         shutdown_cpy.notify_error(true).await;
//             //     });
//             // }
//             Box::pin(async {})
//         },
//     ));
// }

async fn read_track(
    track: Arc<TrackRemote>,
    tx: &Sender<f32>,
    shutdown: shutdown::Shutdown,
) -> Result<(), Error> {
    let mut error_tracker = ErrorTracker::new(READ_TRACK_THRESHOLD, READ_TRACK_LIMIT);
    shutdown.add_task().await;

    let mut decoder = match AudioDecoder::new() {
        Ok(decoder) => decoder,
        Err(e) => {
            log::error!("RECEIVER | Error creating audio decoder: {e}");
            shutdown.notify_error(false).await;
            return Err(Error::new(ErrorKind::Other, "Error creating audio decoder"));
        }
    };

    loop {
        tokio::select! {
            result = track.read_rtp() => {
                if let Ok((rtp_packet, _)) = result {

                    let value = match decoder.decode(rtp_packet.payload.to_vec()){
                        Ok(value) => {
                            error_tracker.increment();
                            value
                        },
                        Err(e) => {
                            if error_tracker.increment_with_error(){
                                log::error!("RECEIVER | Max Attemps | Error decoding RTP packet: {e}");
                                shutdown.notify_error(false).await;
                                return Err(Error::new(ErrorKind::Other, "Error decoding RTP packet"));
                            }else{
                                log::warn!("RECEIVER | Error decoding RTP packet: {e}");
                            }
                            continue
                        }
                    };
                    for v in value {
                        let _ = tx.try_send(v);
                    }
                    error_tracker.increment();
                }else if error_tracker.increment_with_error(){
                        log::error!("RECEIVER | Max Attemps | Error reading RTP packet");
                        shutdown.notify_error(false).await;
                        return Err(Error::new(ErrorKind::Other, "Error reading RTP packet"));
                }else{
                        log::warn!("RECEIVER | Error reading RTP packet");
                };

            }
            _ = tokio::signal::ctrl_c() => {
                return Ok(());
            }
            _= shutdown.wait_for_error() => {
                println!("Se cerro el read track");
                return Ok(());
            }
        }
    }
}

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
                    shutdown_cpy.notify_error(false).await;
                }
            })
        } else if d_label == MOUSE_CHANNEL_LABEL {
            //TODO: HANDLEAR MOUSE CHANNEL
            Box::pin(async {
                
                MouseController::start_mouse_controller(d);
            })
           
        } else if d_label == KEYBOARD_CHANNEL_LABEL {
            //TODO: HANDLEAR KEYBOARD CHANNEL
            Box::pin(async {})
        } else {
            Box::pin(async move {
                log::info!("RECEIVER |New DataChannel has been opened | {d_label}");
            })
        }
    }));
}
