use std::io::Error;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
pub struct FrontConnection {
    rx: mpsc::Receiver<String>,
    tx_disconnect: mpsc::Sender<String>,
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
        let listener = TcpListener::bind("127.0.0.1:".to_string() + port).await?;

        let mut socket = listener.accept().await?.0;
        let (tx, rx) = mpsc::channel(100);
        let (tx_disconnect, mut rx_disconnect) = mpsc::channel(100);

        tokio::spawn(async move {
            let (socket_reader, mut socket_writer) = socket.split();
            let mut reader = BufReader::new(socket_reader);
            loop {
                let mut buffer = Vec::new();

                tokio::select! {
                    msg = rx_disconnect.recv() => {

                        let rec_msg: String = match msg {
                            Some(m) => m,
                            None => continue,
                        };
                        let _ = socket_writer.write(rec_msg.as_bytes()).await;
                    }
                    bytes_read = reader
                    .read_until(b'\n', &mut buffer) => {
                        if bytes_read.expect("Failed to read until newline") == 0 {
                            return;
                        };
                        let msg = String::from_utf8(buffer).expect("Failed to convert to string");
                        let msg = msg.trim_end_matches('\n').to_string();
                        tx.send(msg).await.expect("channel send failed");
                    }
                }
            }
        });
        Ok(FrontConnection { rx, tx_disconnect })
    }

    pub async fn send_ready(&mut self) -> Result<(), Error> {
        if let Err(e) = self.tx_disconnect.send("readyToStart".to_string()).await {
            return Err(Error::new(std::io::ErrorKind::Other, e));
        };
        return Ok(());
    }

    pub async fn read_message(&mut self) -> Result<String, Error> {
        let msg = self.rx.recv().await.expect("Failed to receive message");
        Ok(msg)
    }

    pub async fn waiting_to_start(&mut self) -> Result<Client, Error> {
        loop {
            let msg = match self.rx.recv().await {
                Some(m) => m,
                None => {
                    return Err(Error::new(
                        std::io::ErrorKind::ConnectionAborted,
                        "Flutter disconnected",
                    ))
                } //Significa que flutter se desconecto
            };

            let parts: Vec<&str> = msg.split('|').collect();
            match parts[0] {
                "startOffering" => {
                    let username = parts[1].trim_end_matches('\n').to_string();
                    return Ok(Client {
                        client_type: ClientType::SENDER,
                        username,
                        user_to_connect: None,
                        game_name: None,
                        minutes: None,
                    });
                }
                "startGameWithUser" => {
                    let username = parts[1].to_string();
                    let user_to_connect = parts[2].to_string();
                    let game_name = parts[3].to_string();
                    let minutes = parts[4].trim_end_matches('\n').to_string();
                    return Ok(Client {
                        client_type: ClientType::RECEIVER,
                        username,
                        user_to_connect: Some(user_to_connect),
                        game_name: Some(game_name),
                        minutes: Some(minutes),
                    });
                }
                _ => continue,
            }
        }
    }

    pub async fn waiting_to_disconnect(&mut self) -> Result<(), Error> {
        loop {
            let mut msg = self.read_message().await?;
            msg = msg.trim_end_matches('\n').to_string();
            match msg.as_str() {
                "disconnect" => {
                    return Ok(());
                }
                _ => {
                    print!("Message: {}", msg);
                    continue;
                }
            }
        }
    }
}
