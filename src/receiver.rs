pub mod audio;
pub mod utils;
pub mod webrtcommunication;
use std::io::{Error, ErrorKind};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Notify, Semaphore};

use utils::error_tracker::ErrorTracker;
use utils::webrtc_const::{READ_TRACK_LIMIT, READ_TRACK_THRESHOLD};
use webrtc::data_channel::RTCDataChannel;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::{
    api::media_engine::MIME_TYPE_OPUS, ice_transport::ice_connection_state::RTCIceConnectionState,
    rtcp::payload_feedbacks::picture_loss_indication::PictureLossIndication,
    rtp_transceiver::rtp_codec::RTPCodecType, track::track_remote::TrackRemote,
};

use std::sync::Mutex;
use tokio::sync::mpsc::{error, Receiver, Sender};

use cpal::traits::StreamTrait;

use crate::audio::audio_decoder::AudioDecoder;
use crate::utils::common_utils::get_args;
use crate::utils::latency_const::LATENCY_CHANNEL_LABEL;
use crate::utils::webrtc_const::{ENCODE_BUFFER_SIZE, STUN_ADRESS};
use crate::webrtcommunication::communication::{encode, Communication};
use crate::webrtcommunication::latency::Latency;
use tokio::signal;

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize Log:
    env_logger::builder().format_target(false).init();
    let error_notifier = Arc::new(tokio::sync::Semaphore::new(0));

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

    let notify_tx = Arc::new(Notify::new());
    let notify_rx = notify_tx.clone();

    let peer_connection = comunication.get_peer();

    // Set a handler for when a new remote track starts, this handler saves buffers to disk as
    // an ivf file, since we could have multiple video tracks we provide a counter.
    // In your application this is where you would handle/process video
    set_on_track_handler(
        &peer_connection,
        notify_rx,
        tx_decoder_1,
        error_notifier.clone(),
    );

    channel_handler(&peer_connection);

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

    // Set the handler for ICE connection state
    // This will notify you when the peer has connected/disconnected
    set_on_ice_connection_state_change_handler(&peer_connection, notify_tx, error_notifier.clone());

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
        _ = error_notifier.acquire() => {
            //TODO: debería ser el ultimo que cierra, chequear que todo termino y recien ahi salir
            log::info!("RECEIVER | Error notifier signal");
        }
    };

    if peer_connection.close().await.is_err() {
        return Err(Error::new(
            ErrorKind::Other,
            "Error closing peer connection",
        ));
    }

    error_notifier.close();

    Ok(())
}

fn set_on_track_handler(
    peer_connection: &Arc<RTCPeerConnection>,
    notify_rx: Arc<Notify>,
    tx_decoder_1: Sender<f32>,
    error_notifier: Arc<Semaphore>,
) {
    let pc = Arc::downgrade(peer_connection);

    peer_connection.on_track(Box::new(move |track, _, _| {
        // Send a PLI on an interval so that the publisher is pushing a keyframe every rtcpPLIInterval
        let media_ssrc = track.ssrc();
        let pc2 = pc.clone();
        tokio::spawn(async move {
            let mut result = anyhow::Result::<usize>::Ok(0);
            while result.is_ok() {
                let timeout = tokio::time::sleep(Duration::from_secs(3));
                tokio::pin!(timeout);

                tokio::select! {
                    _ = timeout.as_mut() =>{
                        if let Some(pc) = pc2.upgrade(){
                            result = pc.write_rtcp(&[Box::new(PictureLossIndication{
                                sender_ssrc: 0,
                                media_ssrc,
                            })]).await.map_err(Into::into);
                        }else{
                            break;//TODO: error
                        }
                    }
                };
            }
        });

        let notify_rx2 = Arc::clone(&notify_rx);
        let decoder = match AudioDecoder::new() {
            Ok(decoder) => decoder,
            Err(e) => {
                log::error!("RECEIVER | Error creating audio decoder: {e}");
                return Box::pin(async {});
            }
        };

        let tx_decoder_1_clone = tx_decoder_1.clone();
        let error_notifier_cpy = error_notifier.clone();
        Box::pin(async move {
            let codec = track.codec();
            let mime_type = codec.capability.mime_type.to_lowercase();
            if mime_type == MIME_TYPE_OPUS.to_lowercase() {
                log::info!("RECEIVER | Got OPUS Track");
                tokio::spawn(async move {
                    let _ = read_track(
                        track,
                        notify_rx2,
                        decoder,
                        &tx_decoder_1_clone,
                        error_notifier_cpy,
                    )
                    .await;
                    //TODO: agregar signaling
                });
            }
        })
    }));
}

fn set_on_ice_connection_state_change_handler(
    peer_connection: &Arc<RTCPeerConnection>,
    notify_tx: Arc<Notify>,
    error_notifier: Arc<Semaphore>,
) {
    peer_connection.on_ice_connection_state_change(Box::new(
        move |connection_state: RTCIceConnectionState| {
            log::info!("RECEIVER | ICE Connection State has changed | {connection_state}");

            if connection_state == RTCIceConnectionState::Connected {
                println!("Ctrl+C the remote client to stop the demo");
            } else if connection_state == RTCIceConnectionState::Failed {
                notify_tx.notify_waiters();

                error_notifier.add_permits(1);
            }
            Box::pin(async {})
        },
    ));
}

async fn read_track(
    track: Arc<TrackRemote>,
    notify: Arc<Notify>,
    mut decoder: AudioDecoder,
    tx: &Sender<f32>,
    error_notifier: Arc<Semaphore>,
) -> Result<(), Error> {
    let mut error_tracker = ErrorTracker::new(READ_TRACK_THRESHOLD, READ_TRACK_LIMIT);
    loop {
        //TODO: signaling de error
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
                                error_notifier.add_permits(1);
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
                        error_notifier.add_permits(1);
                        return Err(Error::new(ErrorKind::Other, "Error reading RTP packet"));
                }else{
                        log::warn!("RECEIVER | Error reading RTP packet");
                };

            }
            _ = tokio::signal::ctrl_c() => {
                return Ok(());
            }
            _= error_notifier.acquire() => {
                return Ok(());
            }
            _ = notify.notified() => {
                log::info!("RECEIVER | file closing begin after notified");
            }
        }
    }
}

fn channel_handler(peer_connection: &Arc<RTCPeerConnection>) {
    // Register data channel creation handling
    peer_connection.on_data_channel(Box::new(move |d: Arc<RTCDataChannel>| {
        let d_label = d.label().to_owned();

        if d_label == LATENCY_CHANNEL_LABEL {
            Box::pin(async move {
                // Start the latency measurement
                if let Err(e) = Latency::start_latency_receiver(d).await {
                    log::error!("RECEIVER | Error starting latency receiver: {e}");
                    //TODO: retornar error?
                }
            })
        } else {
            Box::pin(async move {
                log::info!("RECEIVER |New DataChannel has been opened | {d_label}");
            })
        }
    }));
}
