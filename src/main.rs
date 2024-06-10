pub mod front_connection;
pub mod gstreamer_pipeline;
pub mod input;
pub mod output;
pub mod services;
pub mod sound;
pub mod utils;
pub mod video;
pub mod webrtcommunication;
pub mod websocketprotocol;

use crate::front_connection::front_protocol::{ClientType, FrontConnection};
use crate::services::receiver::ReceiverSide;
use crate::services::sender::SenderSide;
use crate::websocketprotocol::socket_protocol::WsProtocol;

use std::io::Error;

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::builder().format_target(false).init();
    // Initialize GStreamer
    gstreamer::init().unwrap();
    let mut ws = WsProtocol::ws_protocol().await?;

    loop {
        println!("Ready to start");
        let mut front_connection = FrontConnection::new("2930").await?;
        let client = front_connection.waiting_to_start().await?;

        match client.client_type {
            ClientType::RECEIVER => {
                let offerer_username = client
                    .user_to_connect
                    .expect("Missing offerer name parameter.");

                let game_name = client.game_name.expect("Missign game name parameter.");
                if (ReceiverSide::init(&client.username, &offerer_username, &game_name).await)
                    .is_err()
                {
                    println!("Connection Missed. \nRestarting...");
                    continue;
                }
                break;
            }
            ClientType::SENDER => {
                if (SenderSide::init(&client.username, &mut ws).await).is_err() {
                    continue;
                }
            }
        }
    }

    println!("Main done");
    unsafe {
        gstreamer::deinit();
    }
    Ok(())
}
