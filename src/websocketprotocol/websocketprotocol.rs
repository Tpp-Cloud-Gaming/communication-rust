
use std::io::{Error, ErrorKind};
use websockets::WebSocket;

pub struct WsProtocol {
    ws: WebSocket,
}


impl WsProtocol {

    pub async fn ws_protocol() -> Result<WsProtocol, Error> {
        let ws = WebSocket::connect("wss://cloud-gaming-server.onrender.com").await;
        match ws {
            Ok(ws) => {
                return Ok(WsProtocol {ws})
            }
            Err(e) => {
                return Err(Error::new(ErrorKind::Other, "Error connecting to the server"));
            }
        }
        
    }
    
    pub async fn init_offer(&mut self, username: &str) -> Result<(), Error> {
        match self.ws.send_text(format!("initOfferer|{}", username)).await {
            Ok(_) => Ok(()),
            Err(e) => {
                return Err(Error::new(ErrorKind::Other, "Error sending offer message"));
            }
        }
        
    }

    
    pub async fn wait_for_game_solicitude(&mut self)-> Result<String, Error> {
        let msg = match self.ws.receive().await {
            Ok(msg) => msg,
            Err(_) => {
                return Err(Error::new(ErrorKind::Other, "Error receiving message"));
            }
        };
        let response = msg.as_text().unwrap().0;
        let parts: Vec<&str> = response.split('|').collect();
        match parts[0] {
            "sdpRequestFrom" => {
                let client_name = parts[1];
                let _game_name = parts[2];
                return Ok(client_name.to_string()); // Eventually also return 
            }
            _ => {
                return Err(Error::new(ErrorKind::InvalidData, "Invalid message"));
            }
        }
    }
    
    
    pub async fn send_sdp_to_client(&mut self, client_name: &str, sdp: &str) -> Result<(), Error> {
        match self.ws.send_text(format!("offererSdp|{}|{}", client_name, sdp)).await{
            Ok(_) => return Ok(()),
            Err(e) => {
                return Err(Error::new(ErrorKind::Other, "Error sending sdp message"));
            }

        };
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
            "sdpClient" => {
                let sdp = parts[1];
                return Ok(sdp.to_string());
                // Do something with the answer
            }
            _ => {
                return Err(Error::new(ErrorKind::InvalidData, "Invalid message"));
            }
        }
    }

    pub async fn initClient(&mut self, username:&str, offerer_username:&str, game_name:&str) -> Result<(), Error> {
        match self.ws.send_text(format!("initClient|{}|{}|{}", username, offerer_username, game_name)).await {
            Ok(_) => return Ok(()),
            Err(e) => {
                return Err(Error::new(ErrorKind::Other, "Error sending init client message"));
            }
        };

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
            "sdpOfferer" => {
                let sdp = parts[1];
                return Ok(sdp.to_string());
            }
            _ => {
                return Err(Error::new(ErrorKind::InvalidData, "Invalid message"));
            }
        }
    }

    pub async fn send_sdp_to_offerer(&mut self, offerer_username: &str, sdp: &str) -> Result<(), Error> {
        match self.ws.send_text(format!("clientSdp|{}|{}", offerer_username, sdp)).await{
            Ok(_) => return Ok(()),
            Err(e) => {
                return Err(Error::new(ErrorKind::Other, "Error sending sdp message"));
            }

        };
    }

}