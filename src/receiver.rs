pub mod audio;
pub mod utils;
pub mod webrtcommunication;

use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use std::io::{Error, ErrorKind};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Notify;

use webrtc::data_channel::RTCDataChannel;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::{
    api::media_engine::MIME_TYPE_OPUS, ice_transport::ice_connection_state::RTCIceConnectionState,
    rtcp::payload_feedbacks::picture_loss_indication::PictureLossIndication,
    rtp_transceiver::rtp_codec::RTPCodecType, track::track_remote::TrackRemote,
};

use std::sync::Mutex;
use tokio::sync::mpsc::{Receiver, Sender};

use cpal::traits::StreamTrait;

use crate::audio::audio_decoder::AudioDecoder;
use crate::utils::latency_const::LATENCY_CHANNEL_LABEL;
use crate::utils::webrtc_const::{ENCODE_BUFFER_SIZE, STUN_ADRESS};
use crate::webrtcommunication::communication::Communication;
use crate::webrtcommunication::latency::Latency;

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize Log:
    env_logger::builder().format_target(false).init();

    let (tx_decoder_1, rx_decoder_1): (Sender<f32>, Receiver<f32>) =
        tokio::sync::mpsc::channel(ENCODE_BUFFER_SIZE);
    let audio_player = audio::audio_player::AudioPlayer::new(
        "Headset Earphone (G435 Wireless Gaming Headset)",
        Arc::new(Mutex::new(rx_decoder_1)),
    )
    .unwrap();
    let stream = audio_player.start().unwrap();
    stream.play().unwrap();

    let comunication = Communication::new(STUN_ADRESS.to_owned()).await?;

    let notify_tx = Arc::new(Notify::new());
    let notify_rx = notify_tx.clone();

    let peer_connection = comunication.get_peer();

    // Set a handler for when a new remote track starts, this handler saves buffers to disk as
    // an ivf file, since we could have multiple video tracks we provide a counter.
    // In your application this is where you would handle/process video
    set_on_track_handler(&peer_connection, notify_rx, tx_decoder_1);

    channel_handler(&peer_connection);

    // Allow us to receive 1 audio track
    if let Err(_) = peer_connection
        .add_transceiver_from_kind(RTPCodecType::Audio, None)
        .await
    {
        return Err(Error::new(
            ErrorKind::Other,
            "Error adding audio transceiver",
        ));
    }

    let (done_tx, mut done_rx) = tokio::sync::mpsc::channel::<()>(1);

    // Set the handler for ICE connection state
    // This will notify you when the peer has connected/disconnected
    set_on_ice_connection_state_change_handler(&peer_connection, notify_tx, done_tx);

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
    if let Err(_) = peer_connection.set_local_description(answer).await {
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
        let b64 = BASE64_STANDARD.encode(&json_str);
        println!("{b64}");
    } else {
        log::error!("RECEIVER | Generate local_description failed!");
    }

    println!("Press ctrl-c to stop");
    tokio::select! {
        _ = done_rx.recv() => {
            log::info!("RECEIVER | Received done signal");
        }
        _ = tokio::signal::ctrl_c() => {
            log::info!("RECEIVER | Received done signal222");
            println!();
        }
    };

    if let Err(_) = peer_connection.close().await {
        return Err(Error::new(
            ErrorKind::Other,
            "Error closing peer connection",
        ));
    }

    Ok(())
}

fn set_on_track_handler(
    peer_connection: &Arc<RTCPeerConnection>,
    notify_rx: Arc<Notify>,
    tx_decoder_1: Sender<f32>,
) {
    let pc = Arc::downgrade(&peer_connection);

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
                            break;
                        }
                    }
                };
            }
        });

        let notify_rx2 = Arc::clone(&notify_rx);
        let decoder = AudioDecoder::new().unwrap();

        let tx_decoder_1_clone = tx_decoder_1.clone();
        Box::pin(async move {
            let codec = track.codec();
            let mime_type = codec.capability.mime_type.to_lowercase();
            if mime_type == MIME_TYPE_OPUS.to_lowercase() {
                log::info!("RECEIVER | Got OPUS Track");

                tokio::spawn(async move {
                    let _ = read_track(track, notify_rx2, decoder, &tx_decoder_1_clone).await;
                });
            }
        })
    }));
}

fn set_on_ice_connection_state_change_handler(
    peer_connection: &Arc<RTCPeerConnection>,
    notify_tx: Arc<Notify>,
    done_tx: Sender<()>,
) {
    peer_connection.on_ice_connection_state_change(Box::new(
        move |connection_state: RTCIceConnectionState| {
            log::info!("RECEIVER | ICE Connection State has changed | {connection_state}");

            if connection_state == RTCIceConnectionState::Connected {
                println!("Ctrl+C the remote client to stop the demo");
            } else if connection_state == RTCIceConnectionState::Failed {
                notify_tx.notify_waiters();

                let _ = done_tx.try_send(());
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
) -> Result<(), ()> {
    loop {
        tokio::select! {
            result = track.read_rtp() => {
                if let Ok((rtp_packet, _)) = result {
                    //println!("Llega Luquitas");
                    let value = decoder.decode(rtp_packet.payload.to_vec()).unwrap();
                    for v in value {
                        let _ = tx.try_send(v);
                    }

                }else{
                    println!("Error leyendo paquete");
                    return Ok(());
                }
            }
            _ = notify.notified() => {
                println!("file closing begin after notified");
                return Ok(());
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
                Latency::start_latency_receiver(d).await.unwrap(); //TODO: sacar unwrap
            })
        } else {
            Box::pin(async move {
                println!("RECEIVER | New DataChannel has been opened | {d_label}");
            })
        }
    }));
}
