use std::io::Error;
use std::sync::Arc;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::sync::Semaphore;
pub struct FrontConnection {
    rx: mpsc::Receiver<Client>,
    ready_start_semaphore: Arc<Semaphore>,
    disconnect_semaphore: Arc<Semaphore>,
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

        let ready_start_semaphore = Arc::new(Semaphore::new(0));
        let disconnect_semaphore = Arc::new(Semaphore::new(0));
        let (tx, rx) = mpsc::channel(100);

        let ready_start_cpy = ready_start_semaphore.clone();
        let disconnect_cpy = disconnect_semaphore.clone();
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
                handle_message(
                    tx.clone(),
                    msg,
                    ready_start_cpy.clone(),
                    disconnect_cpy.clone(),
                )
                .await;
            }
        });

        Ok(FrontConnection {
            rx,
            ready_start_semaphore,
            disconnect_semaphore,
        })
    }

    pub async fn waiting_to_start(&mut self) -> Result<Client, Error> {
        if let Err(_e) = self.ready_start_semaphore.acquire().await {
            return Err(Error::new(
                std::io::ErrorKind::Other,
                "Waiting to start failed.",
            ));
        };

        match self.rx.recv().await {
            Some(client) => Ok(client),
            None => Err(Error::new(
                std::io::ErrorKind::Other,
                "Failed to receive client.",
            )),
        }
    }

    pub async fn waiting_to_disconnect(&mut self) -> Result<(), Error> {
        if let Err(_e) = self.disconnect_semaphore.acquire().await {
            return Err(Error::new(
                std::io::ErrorKind::Other,
                "Waiting to start failed.",
            ));
        };
        Ok(())
    }
}

pub async fn handle_message(
    tx: mpsc::Sender<Client>,
    msg: String,
    ready_start_semaphore: Arc<Semaphore>,
    disconnect_semaphore: Arc<Semaphore>,
) {
    let parts: Vec<&str> = msg.split('|').collect();
    match parts[0] {
        "startOffering" => {
            let username = parts[1].trim_end_matches('\n').to_string();
            let client = Client {
                client_type: ClientType::SENDER,
                username,
                user_to_connect: None,
                game_name: None,
                minutes: None,
            };
            tx.send(client).await.expect("Failed to send client."); //TODO: Handle error
            ready_start_semaphore.add_permits(1);
        }
        "startGameWithUser" => {
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
            ready_start_semaphore.add_permits(1);
        }
        "disconnect" => {
            disconnect_semaphore.add_permits(1);
        }
        _ => (),
    }
}
