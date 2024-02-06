use std::io::{Error, ErrorKind};
use std::net::UdpSocket;
use std::sync::Arc;
use std::time::Duration;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::{data_channel::RTCDataChannel, peer_connection::RTCPeerConnection};

use crate::utils::latency_const::{
    LATENCY_CHANNEL_LABEL, LOOP_LATENCY_TIME, SNTP_POOL_ADDR, UDP_SOCKET_ADDR, UDP_SOCKET_TIMEOUT,
};
//TODO: Cambiar prints por logs

/// Struct to measure the latency between the peers in the Sender or Receiver side
///
/// Uses a data channel to send the messages and a SNTP client to get the time
pub struct Latency {}

impl Latency {
    /// Start the latency in the sender side, create a data channel and send the local time
    pub async fn start_latency_sender(pc: Arc<RTCPeerConnection>) -> Result<(), Error> {
        let latency_channel = match pc.create_data_channel(LATENCY_CHANNEL_LABEL, None).await {
            Ok(ch) => ch,
            Err(_) => {
                return Err(Error::new(
                    ErrorKind::Other,
                    "Error creating latency data channel",
                ))
            }
        };
        println!("Latency Data channel created");
        let socket = create_socket(UDP_SOCKET_ADDR, Duration::from_secs(UDP_SOCKET_TIMEOUT))?;
        // Register channel opening handling
        let d1 = Arc::clone(&latency_channel);
        latency_channel.on_open(Box::new(move || {
            println!("Data channel '{}'-'{}' open. Random messages will now be sent to any connected DataChannels every {} seconds", d1.label(), d1.id(),LOOP_LATENCY_TIME);

            let d2 = Arc::clone(&d1);
            //TODO: Retornar errores ?
            Box::pin(async move {
                loop {
                    let timeout = tokio::time::sleep(Duration::from_secs(LOOP_LATENCY_TIME));
                    let socket_cpy = match socket.try_clone(){
                        Ok(s) => s,
                        Err(e) => {
                            println!("Error cloning socket: {:?}", e);
                            return;
                    }
                    };
                    tokio::pin!(timeout);

                    tokio::select! {
                        _ = timeout.as_mut() => {
                            let time = match get_time(socket_cpy){
                                Ok(t) => t,
                                Err(e) => {
                                    println!("Error getting time: {:?}", e);
                                    return;
                                }
                            };
                            //DESCOMENTAR PARA VER LA HORA QUE MANDA EL SERNDER
                            //println!("Sending '{:?}'", time);
                            if let Err(e) = d2.send_text(time.to_string()).await{
                                println!("Error sending message: {:?}", e);
                                return;
                            };
                        }
                    };
                }
            })
        }));

        Ok(())
    }

    /// Start the latency in the receiver side, handle all the messages of the sender and calculate the latency
    pub async fn start_latency_receiver(ch: Arc<RTCDataChannel>) -> Result<(), Error> {
        ch.on_close(Box::new(move || {
            println!("Data channel closed");
            Box::pin(async {})
        }));

        let socket = create_socket(UDP_SOCKET_ADDR, Duration::from_secs(UDP_SOCKET_TIMEOUT))?;
        //TODO: Retornar errores ?
        // Register text message handling
        ch.on_message(Box::new(move |msg: DataChannelMessage| {
            let socket_cpy = match socket.try_clone() {
                Ok(s) => s,
                Err(e) => {
                    println!("Error cloning socket: {:?}", e);
                    return Box::pin(async {});
                }
            };
            Box::pin(async move {
                let msg_str = match String::from_utf8(msg.data.to_vec()) {
                    Ok(s) => s,
                    Err(e) => {
                        println!("Error converting message to string: {:?}", e);
                        return;
                    }
                };
                let rec_time = match msg_str.parse::<u32>() {
                    Ok(t) => t,
                    Err(e) => {
                        println!("Error parsing message to u32: {:?}", e);
                        return;
                    }
                };
                //DESCOMENTAR PRINTS PARA VER DATA RECIBIDA POR RECEIVER
                //println!("Received time: '{:?}'", rec_time);
                let time = match get_time(socket_cpy) {
                    Ok(t) => t,
                    Err(e) => {
                        println!("Error getting time: {:?}", e);
                        return;
                    }
                };
                //println!("Actual time: '{:?}'", time);
                //convert the difference to milliseconds
                let diff = (time - rec_time) / 1000000 as u32;
                //println!("Difference: {} milliseconds", diff);
            })
        }));

        Ok(())
    }
}

fn create_socket(address: &str, timeout: Duration) -> Result<UdpSocket, Error> {
    let socket = UdpSocket::bind(address)?;
    match socket.set_read_timeout(Some(timeout)) {
        Ok(_) => Ok(socket),
        Err(e) => Err(e),
    }
}

fn get_time(socket: UdpSocket) -> Result<u32, Error> {
    let result = match sntpc::simple_get_time(SNTP_POOL_ADDR, socket) {
        Ok(r) => r,
        Err(e) => {
            println!("Error getting time: {:?}", e);
            return Err(Error::new(ErrorKind::Other, "Error getting time"));
        }
    };
    //DESCOMENTAR PARA VER CUANTO TARDA EL RTT Y LA HORA QUE RECIBE
    //println!("rtt {}, seconds {}", result.roundtrip(), result.sec());

    Ok(sntpc::fraction_to_nanoseconds(result.sec_fraction()) - (result.roundtrip() * 1000) as u32)
}
