mod codecs;
mod webrtcommunication;
mod utils;

use std::{sync::Arc, time::Duration};
use std::io::{Error, ErrorKind};

use base64::prelude::BASE64_STANDARD;
use base64::Engine;

use tokio::sync::Notify;

use webrtc::{rtcp::payload_feedbacks::picture_loss_indication::PictureLossIndication, api::media_engine::MIME_TYPE_OPUS, rtp_transceiver::rtp_codec::RTPCodecType, ice_transport::ice_connection_state::RTCIceConnectionState, track::track_remote::TrackRemote};

use dotenv::dotenv;

use crate::webrtcommunication::communication::Communication;
use crate::codecs::audio_decoder::AudioDecoder;


#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenv().ok();


    let comunication = Communication::new("stun:stun.l.google.com:19302".to_owned()).await?;
    
    let notify_tx = Arc::new(Notify::new());
    let notify_rx = notify_tx.clone();

    let peer_connection = comunication.get_peer();
    // Set a handler for when a new remote track starts, this handler saves buffers to disk as
    // an ivf file, since we could have multiple video tracks we provide a counter.
    // In your application this is where you would handle/process video
    let pc = Arc::downgrade(&peer_connection);
    peer_connection.on_track(Box::new(move |track, _, _| {
        // Send a PLI on an interval so that the publisher is pushing a keyframe every rtcpPLIInterval
        println!("Trackmania");
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

        const PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/recorded.wav");
        let decoder = AudioDecoder::new(PATH).unwrap();
        
        Box::pin(async move {
            let codec = track.codec();
            let mime_type = codec.capability.mime_type.to_lowercase();
            if mime_type == MIME_TYPE_OPUS.to_lowercase() {
                println!("Got Opus track, saving to disk as output.opus (48 kHz, 2 channels)");

                tokio::spawn(async move {
                    let _ = read_track(track, notify_rx2, decoder).await;
                });
            }
        })
    }));


    // Allow us to receive 1 audio track
    if let Err(_) = peer_connection.add_transceiver_from_kind(RTPCodecType::Audio, None).await {
        return Err(Error::new(ErrorKind::Other, "Error adding audio transceiver"));
    }



    let (done_tx, mut done_rx) = tokio::sync::mpsc::channel::<()>(1);

    
    // Set the handler for ICE connection state
    // This will notify you when the peer has connected/disconnected
    peer_connection.on_ice_connection_state_change(Box::new(
        move |connection_state: RTCIceConnectionState| {
            println!("Connection State has changed {connection_state}");

            if connection_state == RTCIceConnectionState::Connected {
                println!("Ctrl+C the remote client to stop the demo");
            } else if connection_state == RTCIceConnectionState::Failed {
                notify_tx.notify_waiters();

                println!("Done writing media files");

                let _ = done_tx.try_send(());
            }
            Box::pin(async {})
        },
    ));
    
    // Set the remote SessionDescription: ACA METER USER INPUT Y PEGAR EL SDP
    // Wait for the offer to be pasted
    comunication.set_sdp().await?;

    // Create an answer
    let answer = match peer_connection.create_answer(None).await {
        Ok(answer) => answer,
        Err(_) => return Err(Error::new(ErrorKind::Other, "Error creating answer")),  
    };

    // Create channel that is blocked until ICE Gathering is complete
    let mut gather_complete = peer_connection.gathering_complete_promise().await;

    // Sets the LocalDescription, and starts our UDP listeners
    if let Err(_) = peer_connection.set_local_description(answer).await {
        return Err(Error::new(ErrorKind::Other, "Error setting local description"))
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
        println!("generate local_description failed!");
    }

    println!("Press ctrl-c to stop");
    tokio::select! {
        _ = done_rx.recv() => {
            println!("received done signal!");
        }
        _ = tokio::signal::ctrl_c() => {
            println!();
        }
    };

    if let Err(_) = peer_connection.close().await {
        return Err(Error::new(ErrorKind::Other, "Error closing peer connection"));
    }

    Ok(())
}

async fn read_track(track: Arc<TrackRemote>, notify: Arc<Notify>, mut decoder: AudioDecoder) -> Result<(), ()> {
    loop {
        tokio::select! {
            result = track.read_rtp() => {
                if let Ok((rtp_packet, _)) = result {
                    println!("LLego LUCAS PAQUETA");

                    decoder.decode(rtp_packet.payload.to_vec());
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