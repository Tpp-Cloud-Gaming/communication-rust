pub mod audio;
pub mod utils;
pub mod webrtcommunication;
pub mod video;
use std::io::{Error, ErrorKind};
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::audio::audio_capture::AudioCapture;
use crate::audio::audio_encoder::AudioEncoder;

use crate::utils::common_utils::get_args;
use crate::utils::shutdown::Shutdown;
use crate::video::video_capture::run;
use crate::webrtcommunication::communication::{encode, Communication};

use tokio::sync::Notify;

use utils::shutdown;
use webrtc::api::media_engine::{MIME_TYPE_H264, MIME_TYPE_OPUS};
use webrtc::ice_transport::ice_connection_state::RTCIceConnectionState;
use webrtc::media::Sample;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::rtp_transceiver::rtp_sender::RTCRtpSender;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::track::track_local::TrackLocal;

use crate::utils::webrtc_const::{
    AUDIO_CHANNELS, AUDIO_SAMPLE_RATE, AUDIO_TRACK_ID, SEND_TRACK_LIMIT, SEND_TRACK_THRESHOLD, STREAM_TRACK_ID, STUN_ADRESS, VIDEO_TRACK_ID
};
use crate::webrtcommunication::latency::Latency;

#[tokio::main]
async fn main() -> Result<(), Error> {
    //Start log
    env_logger::builder().format_target(false).init();
    let shutdown = Shutdown::new();

    //Check for CLI args
    let audio_device = get_args();

    //Create audio frames channels
    let (tx, rx): (Sender<Vec<f32>>, Receiver<Vec<f32>>) = mpsc::channel();

    // Create video frame channels
    let (tx_video, rx_video): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = mpsc::channel();

    let comunication =
        check_error(Communication::new(STUN_ADRESS.to_owned()).await, &shutdown).await?;

    let mut audio_capture = check_error(AudioCapture::new(audio_device, tx), &shutdown).await?;

    let notify_tx = Arc::new(Notify::new());
    let notify_audio = notify_tx.clone();
    let notify_video = notify_tx.clone();

    let _stream = check_error(audio_capture.start(), &shutdown).await?;
    thread::spawn( move || {
        run(tx_video);
    });
    
    
    let (done_tx, mut done_rx) = tokio::sync::mpsc::channel::<()>(1);

    let pc = comunication.get_peer();
    let (rtp_sender, audio_track) = create_track(pc.clone(), shutdown.clone(),MIME_TYPE_OPUS,AUDIO_TRACK_ID).await?;
    let (rtp_video_sender,video_track) = create_track(pc.clone(), shutdown.clone(),MIME_TYPE_H264,VIDEO_TRACK_ID).await?;

    // Start the latency measurement
    check_error(Latency::start_latency_sender(pc.clone()).await, &shutdown).await?;
    
    let shutdown_cpy_1 = shutdown.clone();
    tokio::spawn(async move {
        read_rtcp(shutdown_cpy_1.clone(), rtp_sender).await;
    });
    
    let shutdown_cpy_3 = shutdown.clone();
    tokio::spawn(async move {
        read_rtcp(shutdown_cpy_3.clone(), rtp_video_sender).await;
    });
    
    let shutdown_cpy_2 = shutdown.clone();
    tokio::spawn(async move {
        start_audio_sending(notify_audio, rx, audio_track, shutdown_cpy_2).await;
    });
    
    let shutdown_cpy_4 = shutdown.clone();
    tokio::spawn(async move {
        start_video_sending(notify_video, rx_video, video_track, shutdown_cpy_4).await;
    });
    
    set_peer_events(&pc, notify_tx, done_tx);
    
    // Create an answer to send to the other process
    let offer = match pc.create_offer(None).await {
        Ok(offer) => offer,
        Err(_) => {
            shutdown.notify_error(true).await;
            return Err(Error::new(ErrorKind::Other, "Error creating offer"));
        }
    };
    // Create channel that is blocked until ICE Gathering is complete
    let mut gather_complete = pc.gathering_complete_promise().await;
    
    // Sets the LocalDescription, and starts our UDP listeners
    if let Err(_e) = pc.set_local_description(offer).await {
        shutdown.notify_error(true).await;
        return Err(Error::new(
            ErrorKind::Other,
            "Error setting local description",
        ));
    }

    let _ = gather_complete.recv().await;

    if let Some(local_desc) = pc.local_description().await {
        let json_str = serde_json::to_string(&local_desc)?;
        let b64 = encode(&json_str);
        println!("{b64}");
    } else {
        log::error!("SENDER | Generate local_description failed");
        shutdown.notify_error(true).await;
        return Err(Error::new(
            ErrorKind::Other,
            "Generate local_description failed",
        ));
    }

    check_error(comunication.set_sdp().await, &shutdown).await?;

    println!("Press ctrl-c to stop");
    tokio::select! {
        _ = done_rx.recv() => {
            log::info!("SENDER | Received done signal");
        }
        _ = tokio::signal::ctrl_c() => {
            println!();
        }
        _ = shutdown.wait_for_shutdown() => {
            log::info!("RECEIVER | Error notifier signal");
        }
    };

    if pc.close().await.is_err() {
        return Err(Error::new(
            ErrorKind::Other,
            "Error closing peer connection",
        ));
    }

    check_error(audio_capture.stop(), &shutdown).await?;

    Ok(())
}

async fn create_track(
    pc: Arc<RTCPeerConnection>,
    shutdown: shutdown::Shutdown,
    mime_type: &str,
    track_id: &str
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
            shutdown.notify_error(true).await;
            Err(Error::new(
                ErrorKind::Other,
                "Error setting local description",
            ))
        }
    }
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

async fn check_error<T, E>(result: Result<T, E>, shutdown: &Shutdown) -> Result<T, E> {
    if result.is_err() {
        shutdown.notify_error(true).await;
    }
    result
}

// Read incoming RTCP packets
// Before these packets are returned they are processed by interceptors. For things
// like NACK this needs to be called.
async fn read_rtcp(shutdown: shutdown::Shutdown, rtp_sender: Arc<RTCRtpSender>) {
    shutdown.add_task().await;
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

async fn start_audio_sending(
    notify_audio: Arc<Notify>,
    rx: Receiver<Vec<f32>>,
    audio_track: Arc<TrackLocalStaticSample>,
    shutdown: shutdown::Shutdown,
) {
    shutdown.add_task().await;
    // Wait for connection established
    notify_audio.notified().await;
    let mut encoder = match AudioEncoder::new() {
        Ok(e) => e,
        Err(err) => {
            log::error!("SENDER | Error creating audio encoder | {}", err);
            shutdown.notify_error(false).await;
            return;
        }
    };

    let mut error_tracker =
        utils::error_tracker::ErrorTracker::new(SEND_TRACK_THRESHOLD, SEND_TRACK_LIMIT);

    loop {
        let data = match rx.recv() {
            Ok(d) => {
                error_tracker.increment();
                d
            }
            Err(err) => {
                if error_tracker.increment_with_error() {
                    log::error!(
                        "SENDER | Max attemps | Error receiving audio data | {}",
                        err
                    );
                    shutdown.notify_error(false).await;
                    return;
                } else {
                    log::warn!("SENDER | Error receiving audio data | {}", err);
                };
                continue;
            }
        };
        let encoded_data = match encoder.encode(data) {
            Ok(d) => {
                error_tracker.increment();
                d
            }
            Err(err) => {
                if error_tracker.increment_with_error() {
                    log::error!("SENDER | Max attemps | Error encoding audio | {}", err);
                    shutdown.notify_error(false).await;
                    return;
                } else {
                    log::warn!("SENDER | Error encoding audio | {}", err);
                };
                continue;
            }
        };
        let sample_duration =
            Duration::from_millis((AUDIO_CHANNELS as u64 * 10000000) / AUDIO_SAMPLE_RATE as u64); //TODO: no hardcodear

        if let Err(err) = audio_track
            .write_sample(&Sample {
                data: encoded_data.into(),
                duration: sample_duration,
                ..Default::default()
            })
            .await
        {
            log::warn!("SENDER | Error writing sample | {}", err);
            if error_tracker.increment_with_error() {
                log::error!("SENDER | Max attemps | Error writing sample | {}", err);
                shutdown.notify_error(false).await;
                return;
            } else {
                log::warn!("SENDER | Error writing sample | {}", err);
            };
            continue;
        } else {
            error_tracker.increment();
        }
        if shutdown.check_for_error().await {
            return;
        }
    }
}


async fn start_video_sending(
    notify_video: Arc<Notify>,
    rx: Receiver<Vec<u8>>,
    video_track: Arc<TrackLocalStaticSample>,
    shutdown: shutdown::Shutdown,
) {
    shutdown.add_task().await;
    // Wait for connection established
    // TODO: Esto puede generar delay me parece
    notify_video.notified().await;
    

    let mut error_tracker =
        utils::error_tracker::ErrorTracker::new(SEND_TRACK_THRESHOLD, SEND_TRACK_LIMIT);

    loop {
        let data = match rx.recv() {
            Ok(d) => {
                error_tracker.increment();
                d
            }
            Err(err) => {
                if error_tracker.increment_with_error() {
                    log::error!(
                        "SENDER | Max attemps | Error receiving video data | {}",
                        err
                    );
                    shutdown.notify_error(false).await;
                    return;
                } else {
                    log::warn!("SENDER | Error receiving video data | {}", err);
                };
                continue;
            }
        };
        
        let sample_duration =
            Duration::from_millis(1000 / 30 as u64); //TODO: no hardcodear

        if let Err(err) = video_track
            .write_sample(&Sample {
                data: data.into(),
                duration: sample_duration,
                ..Default::default()
            })
            .await
        {
            log::warn!("SENDER | Error writing sample | {}", err);
            if error_tracker.increment_with_error() {
                log::error!("SENDER | Max attemps | Error writing sample | {}", err);
                shutdown.notify_error(false).await;
                return;
            } else {
                log::warn!("SENDER | Error writing sample | {}", err);
            };
            continue;
        } else {
            error_tracker.increment();
        }
        if shutdown.check_for_error().await {
            return;
        }
    }
}
