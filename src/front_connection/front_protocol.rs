use std::io::Error;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
pub struct FrontConnection {
    rx: mpsc::Receiver<String>,
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
}

impl FrontConnection {
    pub async fn new() -> Result<FrontConnection, Error> {
        let listener = TcpListener::bind("127.0.0.1:2930").await?;
        let (socket, _) = listener.accept().await?;
        let (tx, rx) = mpsc::channel(100);
        tokio::spawn(async move {
            let mut reader = BufReader::new(socket);
            loop {
                let mut buffer = Vec::new();
                let bytes_read = reader
                    .read_until(b'\n', &mut buffer)
                    .await
                    .expect("Failed to read until newline");
                if bytes_read == 0 {
                    return;
                };
                let msg = String::from_utf8(buffer).expect("Failed to convert to string");
                let msg = msg.trim_end_matches('\n').to_string();
                tx.send(msg).await.expect("channel send failed");
            }
        });
        Ok(FrontConnection { rx })
    }

    pub async fn read_message(&mut self) -> Result<String, Error> {
        let msg = self.rx.recv().await.expect("channel recv failed");
        Ok(msg)
    }

    pub async fn waiting_to_start(&mut self) -> Result<Client, Error> {
        loop {
            let msg = self.read_message().await?;
            let parts: Vec<&str> = msg.split('|').collect();
            match parts[0] {
                "startOffering" => {
                    let username = parts[1].trim_end_matches('\n').to_string();
                    return Ok(Client {
                        client_type: ClientType::SENDER,
                        username,
                        user_to_connect: None,
                        game_name: None,
                    });
                }
                "startGameWithUser" => {
                    let username = parts[1].to_string();
                    let user_to_connect = parts[2].to_string();
                    let game_name = parts[3].trim_end_matches('\n').to_string();
                    return Ok(Client {
                        client_type: ClientType::RECEIVER,
                        username,
                        user_to_connect: Some(user_to_connect),
                        game_name: Some(game_name),
                    });
                }
                _ => continue,
            }
        }
    }
}
