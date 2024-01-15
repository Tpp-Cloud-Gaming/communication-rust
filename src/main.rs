mod receiver;
use audio::audio_decoder::AudioDecoder;
use audio::audio_encoder::AudioEncoder;
use std::fs::File;
use std::io::{BufReader, Write};
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::mpsc;
use anyhow::Result;
use base64::Engine;
use tokio::sync::Notify;
use tokio::time::Duration;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::{MediaEngine, MIME_TYPE_OPUS, MIME_TYPE_VP8, MIME_TYPE_VP9};
use webrtc::api::APIBuilder;
use webrtc::ice_transport::ice_connection_state::RTCIceConnectionState;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::media::io::ivf_reader::IVFReader;
use webrtc::media::io::ogg_reader::OggReader;
use webrtc::media::Sample;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::track::track_local::TrackLocal;
use webrtc::Error;
use cpal::traits::StreamTrait;
use dotenv::dotenv;


use base64::prelude::BASE64_STANDARD;

const OGG_PAGE_DURATION: Duration = Duration::from_millis(20);

pub fn must_read_stdin() -> Result<String> {
    let mut line = String::new();

    std::io::stdin().read_line(&mut line)?;
    line = line.trim().to_owned();
    println!();

    Ok(line)
}

pub fn decode(s: &str) -> Result<String> {
    let b = BASE64_STANDARD.decode(s)?;

    //if COMPRESS {
    //    b = unzip(b)
    //}

    let s = String::from_utf8(b)?;
    Ok(s)
}

pub fn encode(b: &str) -> String {
    //if COMPRESS {
    //    b = zip(b)
    //}

    BASE64_STANDARD.encode(b)
}

#[tokio::main]
async fn main() -> Result<()> {
    // Everything below is the WebRTC-rs API! Thanks for using it ❤️.
    dotenv().ok();
    let (tx, rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = mpsc::channel();

    let mut encoder = AudioEncoder::new(
        "SAMSUNG (NVIDIA High Definition Audio)".to_string(),
        tx,
    )?;

    let stream = encoder.start().unwrap();
    stream.play().unwrap();
    
    // Create a MediaEngine object to configure the supported codec
    let mut m = MediaEngine::default();

    m.register_default_codecs()?;

    // Create a InterceptorRegistry. This is the user configurable RTP/RTCP Pipeline.
    // This provides NACKs, RTCP Reports and other features. If you use `webrtc.NewPeerConnection`
    // this is enabled by default. If you are manually managing You MUST create a InterceptorRegistry
    // for each PeerConnection.
    let mut registry = Registry::new();

    // Use the default set of Interceptors
    registry = register_default_interceptors(registry, &mut m)?;

    // Create the API object with the MediaEngine
    let api = APIBuilder::new()
        .with_media_engine(m)
        .with_interceptor_registry(registry)
        .build();

    // Prepare the configuration
    let config = RTCConfiguration {
        ice_servers: vec![RTCIceServer {
            urls: vec!["stun:stun.l.google.com:19302".to_owned()],
            ..Default::default()
        }],
        ..Default::default()
    };

    // Create a new RTCPeerConnection
    let peer_connection = Arc::new(api.new_peer_connection(config).await?);

    let notify_tx = Arc::new(Notify::new());
    let notify_audio = notify_tx.clone();

    let (done_tx, mut done_rx) = tokio::sync::mpsc::channel::<()>(1);
    let audio_done_tx = done_tx.clone();

    // Create a audio track
    let audio_track = Arc::new(TrackLocalStaticSample::new(
        RTCRtpCodecCapability {
            mime_type: MIME_TYPE_OPUS.to_owned(),
            ..Default::default()
        },
        "audio".to_owned(),
        "webrtc-rs".to_owned(),
    ));

    // Add this newly created track to the PeerConnection
    let rtp_sender = peer_connection
        .add_track(Arc::clone(&audio_track) as Arc<dyn TrackLocal + Send + Sync>)
        .await?;

    // Read incoming RTCP packets
    // Before these packets are returned they are processed by interceptors. For things
    // like NACK this needs to be called.
    // tokio::spawn(async move {
    //     let mut rtcp_buf = vec![0u8; 1500];
    //     while let Ok((_, _)) = rtp_sender.read(&mut rtcp_buf).await {}
    //     Result::<()>::Ok(())
    // });

    //let audio_file_name = audio_file.to_owned();
    tokio::spawn(async move {
        // Wait for connection established
        notify_audio.notified().await;

        loop {
            let data = rx.recv().unwrap();
            //let sample_duration = (2 * 10000000) / 48000;
            let sample_duration = Duration::from_millis((2 * 10000000) / 48000);
            audio_track
            .write_sample(&Sample {
                data: data.try_into().unwrap(),
                duration: sample_duration,
                ..Default::default()
            })
            .await?;

            //let _ = audio_done_tx.try_send(());
        }
        

        Result::<()>::Ok(())
    });

    // Set the handler for ICE connection state
    // This will notify you when the peer has connected/disconnected
    peer_connection.on_ice_connection_state_change(Box::new(
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
    peer_connection.on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
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

      // Create an answer to send to the other process
    let answer = peer_connection.create_offer(None).await?;

    
    // Create channel that is blocked until ICE Gathering is complete
    let mut gather_complete = peer_connection.gathering_complete_promise().await;


    // Sets the LocalDescription, and starts our UDP listeners
    peer_connection.set_local_description(answer).await?;

    let _ = gather_complete.recv().await;


    // Sets the LocalDescription, and starts our UDP listeners
    //peer_connection.set_local_description(answer).await?;

    if let Some(local_desc) = peer_connection.local_description().await {
        let json_str = serde_json::to_string(&local_desc)?;
        let b64 = BASE64_STANDARD.encode(&json_str);
        println!("{b64}");
    } else {
        println!("generate local_description failed!");
    }

    println!("Paste the SDP offer from the remote peer:");
     // Wait for the offer to be pasted
    let line = must_read_stdin()?;
    let desc_data = decode(line.as_str())?;
    let offer = serde_json::from_str::<RTCSessionDescription>(&desc_data)?;

    // Set the remote SessionDescription
    peer_connection.set_remote_description(offer).await?;


    
    // // Set the remote SessionDescription: ACA METER USER INPUT Y PEGAR EL SDP
    // // Wait for the offer to be pasted
    // println!("Paste the SDP offer from the remote peer");
    // let line = must_read_stdin()?;
    // let desc_data = decode(line.as_str())?;
    // let offer = serde_json::from_str::<RTCSessionDescription>(&desc_data)?;

    // // Set the remote SessionDescription
    // peer_connection.set_remote_description(offer).await?;

    // // Create an answer
    // let answer = peer_connection.create_answer(None).await?;

    // // Create channel that is blocked until ICE Gathering is complete
    // let mut gather_complete = peer_connection.gathering_complete_promise().await;

    // // Sets the LocalDescription, and starts our UDP listeners
    // peer_connection.set_local_description(answer).await?;

    // // Block until ICE Gathering is complete, disabling trickle ICE
    // // we do this because we only can exchange one signaling message
    // // in a production application you should exchange ICE Candidates via OnICECandidate
    // let _ = gather_complete.recv().await;

    // // Output the answer in base64 so we can paste it in browser
    // if let Some(local_desc) = peer_connection.local_description().await {
    //     // IMPRIMIR SDP EN BASE64
    //     let json_str = serde_json::to_string(&local_desc)?;
    //     let b64 = BASE64_STANDARD.encode(&json_str);
    //     println!("{b64}");
    // } else {
    //     println!("generate local_description failed!");
    // }

    println!("Press ctrl-c to stop");
    tokio::select! {
        _ = done_rx.recv() => {
            println!("received done signal!");
        }
        _ = tokio::signal::ctrl_c() => {
            println!();
        }
    };


    peer_connection.close().await?;
    drop(stream);

    Ok(())
}
