use sntpc::NtpResult;
use std::io::Write;
use std::io::{Error, ErrorKind};
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::{data_channel::RTCDataChannel, peer_connection::RTCPeerConnection};

use crate::utils::latency_const::{
    LATENCY_CHANNEL_LABEL, LOOP_LATENCY_TIME, MAX_SNTP_RETRY, SNTP_POOL_ADDR, SNTP_SEND_SLEEP,
    UDP_SOCKET_ADDR, UDP_SOCKET_TIMEOUT,
};

const LATENCY_CHECK: bool = false;

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
        log::debug!("LATENCY | Latency Data channel created");
        if LATENCY_CHECK {
            let socket = create_socket(UDP_SOCKET_ADDR, Duration::from_secs(UDP_SOCKET_TIMEOUT))?;
            // Register channel opening handling
            let d1 = Arc::clone(&latency_channel);
            latency_channel.on_open(Box::new(move || {
            log::debug!("LATENCY | Data channel '{}'-'{}' open. Random messages will now be sent to any connected DataChannels every {} seconds", d1.label(), d1.id(),LOOP_LATENCY_TIME);
            let d2 = Arc::clone(&d1);
            //TODO: Retornar errores ?
            Box::pin(async move {
                loop {
                    let timeout = tokio::time::sleep(Duration::from_secs(LOOP_LATENCY_TIME));
                    let socket_cpy = match socket.try_clone(){
                        Ok(s) => s,
                        Err(e) => {
                            log::error!("LATENCY | Error cloning socket: {:?}", e);
                            return;
                    }
                    };
                    tokio::pin!(timeout);

                    tokio::select! {
                        _ = timeout.as_mut() => {
                            let time = match get_time(socket_cpy){
                                Ok(t) => t,
                                Err(e) => {
                                    log::error!("LATENCY | Error getting time: {:?}", e);
                                    return;
                                }
                            };
                            if let Err(e) = d2.send_text(time.to_string()).await{
                                log::error!("LATENCY | Error sending message: {:?}", e);
                                return;
                            };
                        }
                    };
                }
            })
            }));
        }

        Ok(())
    }

    /// Start the latency in the receiver side, handle all the messages of the sender and calculate the latency
    pub async fn start_latency_receiver(ch: Arc<RTCDataChannel>) -> Result<(), Error> {
        ch.on_close(Box::new(move || {
            log::debug!("LATENCY | Data channel is closed");
            Box::pin(async {})
        }));

        let socket = create_socket(UDP_SOCKET_ADDR, Duration::from_secs(UDP_SOCKET_TIMEOUT))?;

        // Create the log file
        let now = chrono::Local::now();
        let date = now.format("%Y-%m-%d_%H-%M-%S").to_string();
        let mut file = match std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(format!("{}.txt", date))
        {
            Ok(f) => f,
            Err(e) => {
                log::error!("LATENCY | Error opening file: {:?}", e);
                return Err(Error::new(ErrorKind::Other, "Error opening file"));
            }
        };

        // Register text message handling
        let file = Arc::new(Mutex::new(file));
        ch.on_message(Box::new(move |msg: DataChannelMessage| {
            let file = Arc::clone(&file);
            let socket_cpy = match socket.try_clone() {
                Ok(s) => s,
                Err(e) => {
                    log::error!("LATENCY | Error cloning socket: {:?}", e);
                    return Box::pin(async {});
                }
            };
            Box::pin(async move {
                let msg_str = match String::from_utf8(msg.data.to_vec()) {
                    Ok(s) => s,
                    Err(e) => {
                        log::error!("LATENCY | Error converting message to string: {:?}", e);
                        return;
                    }
                };
                let rec_time = match msg_str.parse::<u32>() {
                    Ok(t) => t,
                    Err(e) => {
                        log::error!("LATENCY |Error parsing message to u32: {:?}", e);
                        return;
                    }
                };
                let time = match get_time(socket_cpy) {
                    Ok(t) => t,
                    Err(e) => {
                        log::error!("LATENCY |Error getting time: {:?}", e);
                        return;
                    }
                };
                let result = match time.checked_sub(rec_time) {
                    None => {
                        log::error!("LATENCY | Error calculating difference");
                        return;
                    }
                    Some(t) => t,
                };
                log::info!("LATENCY CHECK | Difference: {} milliseconds", result);
                let mut file = file.lock().unwrap();
                if let Err(_e) = write_in_file(&mut *file, result) {
                    return;
                };
            })
        }));

        Ok(())
    }
}

fn write_in_file(file: &mut std::fs::File, latency: u32) -> Result<(), Error> {
    // Get current date and time
    let now = chrono::Local::now();

    // Write to the file
    match writeln!(file, "{},{}", now.to_rfc3339(), latency) {
        Ok(_) => (),
        Err(e) => log::error!("LATENCY | Error writing to file: {:?}", e),
    };
    return Ok(());
}

/// Creates a UDP socket binded to the specified address with a read timeout.
///
/// # Arguments
///
/// * `address` - A string representing the address to bind the socket to.
/// * `timeout` - The duration of the read timeout for the socket.
///
/// # Returns
///
/// A Result containing the created UDP socket on success. Otherwhise an Error is returned.
fn create_socket(address: &str, timeout: Duration) -> Result<UdpSocket, Error> {
    let socket = UdpSocket::bind(address)?;
    match socket.set_read_timeout(Some(timeout)) {
        Ok(_) => Ok(socket),
        Err(e) => Err(e),
    }
}

/// Calculates the latency.
///
/// # Arguments
///
/// * `socket` - A UDP socket used for communication with the SNTP server.
///
/// # Returns
///
/// A [`Result`] containing the calculated latency in milliseconds on success.
/// On failure, returns an [`Error`] indicating the cause of the error.
fn get_time(socket: UdpSocket) -> Result<u32, Error> {
    let result = get_time_from_sntp(socket)?;

    let secs_str = result.sec().to_string();
    let last_two_digits_str = &secs_str[secs_str.len() - 2..];
    let last_two_digits = match last_two_digits_str.parse::<u32>() {
        Ok(t) => t,
        Err(e) => {
            log::error!("LATENCY | Error parsing last two digits: {:?}", e);
            return Err(Error::new(
                ErrorKind::Other,
                "Error parsing last two digits",
            ));
        }
    };

    if last_two_digits == 0 {
        log::info!("LATENCY | Last two digits are 0");
        return Ok(0);
    }

    let mut _secs_in_milis: u32 = 0;
    if let Some(t) = last_two_digits.checked_mul(1000) {
        _secs_in_milis = t;
    } else {
        //Overflow detected
        log::info!("LATENCY | Overflow when multiplying last two digits by 1000");
        return Ok(0);
    }

    let mut _rtt_in_milis: u64 = 0;
    if let Some(t) = result.roundtrip().checked_div(1000) {
        _rtt_in_milis = t;
    } else {
        log::info!("LATENCY | Overflow when dividing roundtrip by 1000");
        return Ok(0);
    };

    Ok(
        (_secs_in_milis + sntpc::fraction_to_milliseconds(result.sec_fraction()))
            - _rtt_in_milis as u32,
    )
}

/// Gets time from the SNTP server.
///
/// # Arguments
///
/// * `socket` - A UDP socket used for communication with the SNTP server.
///
/// # Returns
///
/// A `Result` containing the `NtpResult`.
/// On failure, returns an `Error` indicating the cause of the error.
fn get_time_from_sntp(socket: UdpSocket) -> Result<NtpResult, Error> {
    let mut retry = 0;
    let mut result: NtpResult = NtpResult::new(0, 0, 0, 0, 0, 0);

    // If http request fails, retry max_retry times
    while retry < MAX_SNTP_RETRY {
        let socket_clone = match socket.try_clone() {
            Ok(s) => s,
            Err(e) => {
                log::error!("LATENCY | Error cloning socket: {:?}", e);
                return Err(Error::new(ErrorKind::Other, "Error cloning socket"));
            }
        };
        if let Ok(r) = sntpc::simple_get_time(SNTP_POOL_ADDR, socket_clone) {
            result = r;
            break;
        } else {
            retry += 1;
            sleep(Duration::from_millis(SNTP_SEND_SLEEP));
        }
    }
    if retry == MAX_SNTP_RETRY {
        return Err(Error::new(
            ErrorKind::Other,
            "Error getting time from SNTP server",
        ));
    }
    Ok(result)
}
