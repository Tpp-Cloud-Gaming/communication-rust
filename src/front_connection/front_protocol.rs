use std::io::Error;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::front_connection::front_protocol_const::*;
pub struct FrontConnection {
    rx: mpsc::Receiver<Client>,
    rx_disconnect: mpsc::Receiver<bool>,
}

pub enum ClientType {
    SENDER,
    RECEIVER,
}

pub struct Client {
    pub client_type: ClientType,
    pub username: String,
    pub user_to_connect: Option<String>,
    pub game_name: Option<String>,
    pub minutes: Option<String>,
}

impl FrontConnection {
    pub async fn new(port: &str) -> Result<FrontConnection, Error> {
        let listener = TcpListener::bind(FRONT_IP.to_string() + port).await?;

        let mut socket = listener.accept().await?.0;

        let (tx, rx) = mpsc::channel(100);
        let (tx_disconnect, rx_disconnect) = mpsc::channel(100);

        tokio::spawn(async move {
            let (socket_reader, _socket_writer) = socket.split();
            let mut reader = BufReader::new(socket_reader);
            loop {
                let mut buffer = Vec::new();
                let bytes_read = reader.read_until(b'\n', &mut buffer).await;
                if bytes_read.expect("Failed to read until newline") == 0 {
                    return;
                };
                let msg = String::from_utf8(buffer).expect("Failed to convert to string");
                let msg = msg.trim_end_matches('\n').to_string();
                handle_message(tx.clone(), tx_disconnect.clone(), msg).await;
            }
        });

        Ok(FrontConnection { rx, rx_disconnect })
    }

    pub async fn waiting_to_start(&mut self) -> Result<Client, Error> {
        match self.rx.recv().await {
            Some(client) => Ok(client),
            None => Err(Error::new(
                std::io::ErrorKind::Other,
                "Failed to receive client.",
            )),
        }
    }

    pub async fn waiting_to_disconnect(&mut self) -> Result<(), Error> {
        match self.rx_disconnect.recv().await {
            Some(_b) => Ok(()),
            None => Err(Error::new(
                std::io::ErrorKind::Other,
                "Failed to receive disconnect.",
            )),
        }
    }
}

pub async fn handle_message(
    tx: mpsc::Sender<Client>,
    tx_disconnect: mpsc::Sender<bool>,
    msg: String,
) {
    let parts: Vec<&str> = msg.split('|').collect();
    match parts[0] {
        START_OFFERING_MSG => {
            let username = parts[1].trim_end_matches('\n').to_string();
            let client = Client {
                client_type: ClientType::SENDER,
                username,
                user_to_connect: None,
                game_name: None,
                minutes: None,
            };
            tx.send(client).await.expect("Failed to send client."); //TODO: Handle error
        }
        START_GAME_MSG => {
            let username = parts[1].to_string();
            let user_to_connect = parts[2].to_string();
            let game_name = parts[3].to_string();
            let minutes = parts[4].trim_end_matches('\n').to_string();
            let client = Client {
                client_type: ClientType::RECEIVER,
                username,
                user_to_connect: Some(user_to_connect),
                game_name: Some(game_name),
                minutes: Some(minutes),
            };
            tx.send(client).await.expect("Failed to send client."); //TODO: Handle error
        }
        DISCONNECT_MSG => {
            tx_disconnect
                .send(true)
                .await
                .expect("Failed to send disconnect.");
        }
        _ => (),
    }
}
