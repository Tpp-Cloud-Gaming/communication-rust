pub mod audio;
pub mod utils;
pub mod webrtcommunication;

use std::io::{Error, ErrorKind};
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use crate::audio::audio_capture::AudioCapture;
use crate::audio::audio_encoder::AudioEncoder;

use crate::webrtcommunication::communication::Communication;

use base64::prelude::BASE64_STANDARD;
use base64::Engine;

use tokio::sync::Notify;

use webrtc::api::media_engine::MIME_TYPE_OPUS;
use webrtc::ice_transport::ice_connection_state::RTCIceConnectionState;
use webrtc::media::Sample;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::track::track_local::TrackLocal;

use crate::utils::webrtc_const::{CHANNELS, SAMPLE_RATE, STREAM_TRACK_ID, STUN_ADRESS, TRACK_ID};
use crate::webrtcommunication::latency::Latency;

#[tokio::main]
async fn main() -> Result<(), Error> {
    //Create video frames channels
    env_logger::builder().format_target(false).init();

    let (tx, rx): (Sender<Vec<f32>>, Receiver<Vec<f32>>) = mpsc::channel();

    let comunication = Communication::new(STUN_ADRESS.to_owned()).await?;

    let mut audio_capture = AudioCapture::new(
        "Headset Earphone (G435 Wireless Gaming Headset)".to_string(),
        tx,
    )?;

    let notify_tx = Arc::new(Notify::new());
    let notify_audio = notify_tx.clone();

    //NO BORRAR
    let _stream = audio_capture.start()?;
    //stream.play().unwrap();

    let (done_tx, mut done_rx) = tokio::sync::mpsc::channel::<()>(1);

    let pc = comunication.get_peer();
    let audio_track = create_audio_track();

    // Start the latency measurement
    Latency::start_latency_sender(pc.clone()).await?;

    //let rtp_sender = create_tracks(&pc, audio_track.clone()).await?;
    let rtp_sender = match pc
        .add_track(Arc::clone(&audio_track) as Arc<dyn TrackLocal + Send + Sync>)
        .await
    {
        Ok(rtp_sender) => rtp_sender,
        Err(_) => {
            return Err(Error::new(
                ErrorKind::Other,
                "Error setting local description",
            ))
        }
    };

    // Read incoming RTCP packets
    // Before these packets are returned they are processed by interceptors. For things
    // like NACK this needs to be called.
    tokio::spawn(async move {
        let mut rtcp_buf = vec![0u8; 1500];
        while let Ok((_, _)) = rtp_sender.read(&mut rtcp_buf).await {}
        anyhow::Result::<()>::Ok(())
    });

    tokio::spawn(async move {
        // Wait for connection established
        notify_audio.notified().await;
        let mut encoder = AudioEncoder::new().unwrap();

        loop {
            let data = rx.recv().unwrap();
            let encoded_data = match encoder.encode(data) {
                Ok(d) => d,
                Err(err) => {
                    log::error!("SENDER | Error encoding audio | {}", err);
                    continue;
                }
            };
            let sample_duration =
                Duration::from_millis((CHANNELS as u64 * 10000000) / SAMPLE_RATE as u64); //TODO: no hardcodear

            if let Err(err) = audio_track
                .write_sample(&Sample {
                    data: encoded_data.into(),
                    duration: sample_duration,
                    ..Default::default()
                })
                .await
            {
                log::error!("SENDER | Error writing sample | {}", err);
            }
        }
    });

    set_peer_events(&pc, notify_tx, done_tx);

    // Create an answer to send to the other process
    let offer = match pc.create_offer(None).await {
        Ok(offer) => offer,
        Err(_) => return Err(Error::new(ErrorKind::Other, "Error creating offer")),
    };

    // Create channel that is blocked until ICE Gathering is complete
    let mut gather_complete = pc.gathering_complete_promise().await;

    // Sets the LocalDescription, and starts our UDP listeners
    if let Err(_e) = pc.set_local_description(offer).await {
        return Err(Error::new(
            ErrorKind::Other,
            "Error setting local description",
        ));
    }

    let _ = gather_complete.recv().await;

    if let Some(local_desc) = pc.local_description().await {
        let json_str = serde_json::to_string(&local_desc)?;
        let b64 = BASE64_STANDARD.encode(json_str);
        println!("{b64}");
    } else {
        log::error!("SENDER | Generate local_description failed");
    }

    comunication.set_sdp().await?;

    println!("Press ctrl-c to stop");
    tokio::select! {
        _ = done_rx.recv() => {
            log::info!("SENDER | Received done signal");
        }
        _ = tokio::signal::ctrl_c() => {
            println!();
        }
    };

    if pc.close().await.is_err() {
        return Err(Error::new(
            ErrorKind::Other,
            "Error closing peer connection",
        ));
    }

    audio_capture.stop()?;

    Ok(())
}

fn create_audio_track() -> Arc<TrackLocalStaticSample> {
    Arc::new(TrackLocalStaticSample::new(
        RTCRtpCodecCapability {
            mime_type: MIME_TYPE_OPUS.to_owned(),
            ..Default::default()
        },
        TRACK_ID.to_owned(),
        STREAM_TRACK_ID.to_owned(),
    ))
}

fn set_peer_events(
    pc: &Arc<RTCPeerConnection>,
    notify_tx: Arc<Notify>,
    done_tx: tokio::sync::mpsc::Sender<()>,
) {
    // Set the handler for ICE connection state
    // This will notify you when the peer has connected/disconnected
    pc.on_ice_connection_state_change(Box::new(move |connection_state: RTCIceConnectionState| {
        log::info!("SENDER | ICE Connection State has changed | {connection_state}");
        if connection_state == RTCIceConnectionState::Connected {
            notify_tx.notify_waiters();
        }
        Box::pin(async {})
    }));

    // Set the handler for Peer connection state
    // This will notify you when the peer has connected/disconnected
    pc.on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
        log::info!("Peer Connection State has changed {s}");

        if s == RTCPeerConnectionState::Failed {
            // Wait until PeerConnection has had no network activity for 30 seconds or another failure. It may be reconnected using an ICE Restart.
            // Use webrtc.PeerConnectionStateDisconnected if you are interested in detecting faster timeout.
            // Note that the PeerConnection may come back from PeerConnectionStateDisconnected.
            log::error!("SENDER | Peer connection failed");
            let _ = done_tx.try_send(());
        }

        Box::pin(async {})
    }));
}
