mod webrtcommunication;
mod audio;
mod utils;

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::mpsc;
use std::time::Duration;
use std::io::{Error, ErrorKind};

use crate::audio::audio_capture::AudioCapture;
use crate::audio::audio_encoder::AudioEncoder;

use crate::webrtcommunication::communication::Communication;

use base64::prelude::BASE64_STANDARD;
use base64::Engine;


use cpal::traits::StreamTrait;
use tokio::sync::Notify;

use webrtc::api::media_engine::MIME_TYPE_OPUS;
use webrtc::ice_transport::ice_connection_state::RTCIceConnectionState;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::TrackLocal;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::media::Sample;

use dotenv::dotenv;

#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenv().ok();

    //Create video frames channels
    let (tx, rx): (Sender<Vec<f32>>, Receiver<Vec<f32>>) = mpsc::channel();

    let comunication = Communication::new("stun:stun.l.google.com:19302".to_owned()).await?;

    let mut audio_capture = AudioCapture::new("Altavoces (High Definition Audio Device)".to_string(),tx)?;
    
    // Si no nos guardamos el stream se traba
    let stream = audio_capture.start()?;
    //stream.play().unwrap();

    let notify_tx = Arc::new(Notify::new());
    let notify_audio = notify_tx.clone();

    let (done_tx, mut done_rx) = tokio::sync::mpsc::channel::<()>(1);
    let audio_done_tx = done_tx.clone();

    let audio_track = create_audio_track();

    let pc = comunication.get_peer();

    // // Add this newly created track to the PeerConnection
    let rtp_sender = match pc.add_track(Arc::clone(&audio_track) as Arc<dyn TrackLocal + Send + Sync>).await {
        Ok(rtp_sender) => rtp_sender,
        Err(_) => return Err(Error::new(ErrorKind::Other, "Error setting local description")),
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
            let encoded_data = encoder.encode(data).unwrap();
            let sample_duration = Duration::from_millis((2 * 10000000) / 48000);//TODO: no hardcodear

            
            audio_track
            .write_sample(&Sample {
                data: encoded_data.try_into().unwrap(),
                duration: sample_duration,
                ..Default::default()
            })
            .await.unwrap();//TODO: sacar unwrap
            

            //let _ = audio_done_tx.try_send(());
        }
    });
    //    let a = start_sender(audio_track, notify_audio, rx);


    set_peer_events(&pc, notify_tx, done_tx);

    // Create an answer to send to the other process
    let offer = match pc.create_offer(None).await {
        Ok(offer) => offer,
        Err(_) => return Err(Error::new(ErrorKind::Other, "Error creating offer")),
    };
    
    // Create channel that is blocked until ICE Gathering is complete
    let mut gather_complete = pc.gathering_complete_promise().await;
    
    // Sets the LocalDescription, and starts our UDP listeners
    if  let Err(_e) = pc.set_local_description(offer).await {
        return Err(Error::new(ErrorKind::Other, "Error setting local description"));
    }

    let _ = gather_complete.recv().await;

    if let Some(local_desc) = pc.local_description().await {
        let json_str = serde_json::to_string(&local_desc)?;
        let b64 = BASE64_STANDARD.encode(&json_str);
        println!("{b64}");
    } else {
        println!("generate local_description failed!");
    }

    comunication.set_sdp().await?;
    
    println!("Press ctrl-c to stop");
    tokio::select! {
        _ = done_rx.recv() => {
            println!("received done signal!");
        }
        _ = tokio::signal::ctrl_c() => {
            println!();
        }
    };


    if let Err(_) = pc.close().await {
        return Err(Error::new(ErrorKind::Other, "Error closing peer connection"));
    }

    audio_capture.stop()?;

    Ok(())
}

fn create_audio_track() -> Arc<TrackLocalStaticSample>{
    Arc::new(TrackLocalStaticSample::new(
        RTCRtpCodecCapability {
            mime_type: MIME_TYPE_OPUS.to_owned(),
            ..Default::default()
        },
        "audio".to_owned(),
        "webrtc-rs".to_owned(),
    ))
}


async fn start_sender(audio_track: Arc<TrackLocalStaticSample>, notify_audio: Arc<Notify>, rx: Receiver<Vec<u8>> ) -> Result<(), Error>{
    //let audio_file_name = audio_file.to_owned();
    tokio::spawn(async move {
        // Wait for connection established
        notify_audio.notified().await;

        loop {
            println!("MANDALORIAN");
            let data = rx.recv().unwrap();
            let sample_duration = Duration::from_millis((2 * 10000000) / 48000);//TODO: no hardcodear
            audio_track
            .write_sample(&Sample {
                data: data.try_into().unwrap(),
                duration: sample_duration,
                ..Default::default()
            })
            .await.unwrap();//TODO: sacar unwrap

            //let _ = audio_done_tx.try_send(());
        }
    });
    Ok(())
}



fn set_peer_events(pc: &Arc<RTCPeerConnection>, notify_tx: Arc<Notify>, done_tx: tokio::sync::mpsc::Sender<()>){
    // Set the handler for ICE connection state
    // This will notify you when the peer has connected/disconnected
    pc.on_ice_connection_state_change(Box::new(
        move |connection_state: RTCIceConnectionState| {
            println!("Connection State has changed {connection_state}");
            if connection_state == RTCIceConnectionState::Connected {
                notify_tx.notify_waiters();
            }
            Box::pin(async {})
        },
    ));

    // Set the handler for Peer connection state
    // This will notify you when the peer has connected/disconnected
    pc.on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
        println!("Peer Connection State has changed: {s}");

        if s == RTCPeerConnectionState::Failed {
            // Wait until PeerConnection has had no network activity for 30 seconds or another failure. It may be reconnected using an ICE Restart.
            // Use webrtc.PeerConnectionStateDisconnected if you are interested in detecting faster timeout.
            // Note that the PeerConnection may come back from PeerConnectionStateDisconnected.
            println!("Peer Connection has gone to failed exiting");
            let _ = done_tx.try_send(());
        }

        Box::pin(async {})
    }));
}