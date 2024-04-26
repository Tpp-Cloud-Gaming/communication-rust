pub mod input;
pub mod output;
pub mod sound;
pub mod utils;
pub mod video;
pub mod webrtcommunication;
pub mod websocketprotocol;
pub mod services;
pub mod front_connection;

use crate::services::receiver::ReceiverSide;
use crate::services::sender::SenderSide;
use crate::front_connection::front_protocol::{FrontConnection, ClientType};

use std::io::Error;

#[tokio::main]
async fn main() -> Result<(), Error> {
    
    let mut front_connection = FrontConnection::new().await?;

    let client = front_connection.waiting_to_start().await?;

    match client.client_type {
        ClientType::RECEIVER => {
            let offerer_username = client.user_to_connect.expect("Missing offerer name parameter.");
            let game_name = client.game_name.expect("Missign game name parameter.");
            ReceiverSide::new(&client.username,&offerer_username, &game_name).await?;

        },
        ClientType::SENDER => {
            SenderSide::new(&client.username).await?;
        },
    }

    Ok(())
}