use std::io::{Error, ErrorKind};
use websockets::WebSocket;

use super::socket_protocol_const::*;

pub struct WsProtocol {
    ws: WebSocket,
}

/// Represents the info of the client trying to connect to a sender service
#[derive(Debug)]
pub struct ClientInfo {
    pub client_name: String,
    pub game_name: String,
    pub game_path: String,
    pub minutes: String,
}

impl WsProtocol {
    pub async fn ws_protocol() -> Result<WsProtocol, Error> {
        let ws = WebSocket::connect(SOCKET_URL).await;
        match ws {
            Ok(ws) => Ok(WsProtocol { ws }),
            Err(_) => Err(Error::new(
                ErrorKind::Other,
                "Error connecting to the server",
            )),
        }
    }

    pub async fn init_offer(&mut self, username: &str) -> Result<(), Error> {
        match self
            .ws
            .send_text(format!("{}|{}", INIT_OFFERER_MSG, username))
            .await
        {
            Ok(_) => Ok(()),
            Err(_) => Err(Error::new(ErrorKind::Other, "Error sending offer message")),
        }
    }

    pub async fn wait_for_game_solicitude(&mut self) -> Result<ClientInfo, Error> {
        let msg = match self.ws.receive().await {
            Ok(msg) => msg,
            Err(_) => {
                return Err(Error::new(ErrorKind::Other, "Error receiving message"));
            }
        };
        let response = msg.as_text().unwrap().0;
        let parts: Vec<&str> = response.split('|').collect();
        match parts[0] {
            SDP_REQUEST_FROM_MSG => Ok(ClientInfo {
                client_name: parts[1].to_owned(),
                game_name: parts[2].to_owned(),
                game_path: parts[3].to_owned(),
                minutes: parts[4].to_owned(),
            }),
            _ => Err(Error::new(ErrorKind::InvalidData, "Should be sdp request.")),
        }
    }

    pub async fn send_sdp_to_client(&mut self, client_name: &str, sdp: &str) -> Result<(), Error> {
        match self
            .ws
            .send_text(format!("{}|{}|{}", OFFERER_SDP_MSG, client_name, sdp))
            .await
        {
            Ok(_) => Ok(()),
            Err(_) => Err(Error::new(ErrorKind::Other, "Error sending sdp message")),
        }
    }

    pub async fn wait_for_client_sdp(&mut self) -> Result<String, Error> {
        let msg = match self.ws.receive().await {
            Ok(msg) => msg,
            Err(_) => {
                return Err(Error::new(ErrorKind::Other, "Error receiving message"));
            }
        };
        let response = msg.as_text().unwrap().0;
        let parts: Vec<&str> = response.split('|').collect();
        match parts[0] {
            SDP_CLIENT_MSG => {
                let sdp = parts[1];
                Ok(sdp.to_string())
                // Do something with the answer
            }
            _ => Err(Error::new(ErrorKind::InvalidData, "Should be client sdp")),
        }
    }

    pub async fn init_client(
        &mut self,
        username: &str,
        offerer_username: &str,
        game_name: &str,
        minutes: &str,
    ) -> Result<(), Error> {
        match self
            .ws
            .send_text(format!(
                "{}|{}|{}|{}|{}",
                INIT_CLIENT_MSG, username, offerer_username, game_name, minutes
            ))
            .await
        {
            Ok(_) => Ok(()),
            Err(_) => Err(Error::new(
                ErrorKind::Other,
                "Error sending init client message",
            )),
        }
    }

    pub async fn wait_for_offerer_sdp(&mut self) -> Result<String, Error> {
        let msg = match self.ws.receive().await {
            Ok(msg) => msg,
            Err(_) => {
                return Err(Error::new(ErrorKind::Other, "Error receiving message"));
            }
        };
        let response = msg.as_text().unwrap().0;
        let parts: Vec<&str> = response.split('|').collect();
        match parts[0] {
            SDP_OFFERER_MSG => {
                let sdp = parts[1];
                Ok(sdp.to_string())
            }
            _ => Err(Error::new(ErrorKind::InvalidData, "Should be offerer sdp")),
        }
    }

    pub async fn send_sdp_to_offerer(
        &mut self,
        offerer_username: &str,
        sdp: &str,
    ) -> Result<(), Error> {
        match self
            .ws
            .send_text(format!("{}|{}|{}", CLIENT_SDP_MSG, offerer_username, sdp))
            .await
        {
            Ok(_) => Ok(()),
            Err(_) => Err(Error::new(ErrorKind::Other, "Error sending sdp message")),
        }
    }

    pub async fn start_session(
        &mut self,
        offerer: &str,
        client: &str,
        minutes: &str,
    ) -> Result<(), Error> {
        match self
            .ws
            .send_text(format!(
                "{}|{}|{}|{}",
                START_SESSION_MSG, offerer, client, minutes
            ))
            .await
        {
            Ok(_) => Ok(()),
            Err(_) => Err(Error::new(ErrorKind::Other, "Error sending sdp message")),
        }
    }

    pub async fn force_stop_session(&mut self, username: &str) -> Result<(), Error> {
        match self
            .ws
            .send_text(format!("{}|{}", FORCE_STOP_SESSION_MSG, username))
            .await
        {
            Ok(_) => Ok(()),
            Err(_) => Err(Error::new(
                ErrorKind::Other,
                "Error sending force_stop_session message",
            )),
        }
    }

    pub async fn close_connection(&mut self) -> Result<(), Error> {
        if self.ws.close(None).await.is_err() {
            Err(Error::new(ErrorKind::Other, "Error closing connection"))
        } else {
            Ok(())
        }
    }

    pub async fn wait_for_stop_session(&mut self) -> Result<(), Error> {
        loop {
            let msg = match self.ws.receive().await {
                Ok(msg) => msg,
                Err(_) => {
                    return Err(Error::new(ErrorKind::Other, "Error receiving message"));
                }
            };
            let response = msg.as_text().unwrap().0;
            let parts: Vec<&str> = response.split('|').collect();
            match parts[0] {
                NOTIF_END_SESSION_MSG => return Ok(()),
                _ => {
                    log::info!(
                        "wait_for_stop_session | Received unknown message: {}",
                        parts[0]
                    );
                    continue;
                }
            }
        }
    }
}
