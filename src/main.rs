pub mod front_connection;
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

use std::io::Error;

#[tokio::main]
async fn main() -> Result<(), Error> {
    
    loop {

        let mut front_connection = FrontConnection::new().await?;
            
        let client = front_connection.waiting_to_start().await?;
        
        match client.client_type {
            ClientType::RECEIVER => {
                let offerer_username = client
                    .user_to_connect
                    .expect("Missing offerer name parameter.");
                let game_name = client.game_name
                    .expect("Missign game name parameter.");
            if let Err(_) = ReceiverSide::new(&client.username, &offerer_username, &game_name).await {
                println!("Connection Missed. \nRestarting...");
                continue;
            }
        }
        ClientType::SENDER => {
            if let Err(_) = SenderSide::new(&client.username).await {
                println!("Connection Missed. \nRestarting...");
                continue;
            }
        }
        }
    
    }
}
