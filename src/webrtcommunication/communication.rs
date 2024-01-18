//use crate::utils::utilss::must_read_stdin;

use crate::utils::utils::must_read_stdin;

use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use std::io::{Error, ErrorKind};
use std::sync::Arc;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::{MediaEngine, MIME_TYPE_OPUS};
use webrtc::api::{APIBuilder, API};
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtp_transceiver::rtp_codec::{
    RTCRtpCodecCapability, RTCRtpCodecParameters, RTPCodecType,
};

/// Represents the WebRtc connection with other peer
///
/// Allows as to configure the different stages to establish the connection
pub struct Communication {
    /// 
    peer_connection: Arc<RTCPeerConnection>,
}
impl Communication {
    /// Create new Comunication, needs a correct stun server adress to work
    pub async fn new(stun_adress: String) -> Result<Self, Error> {
        let api = create_api()?;

        let config = RTCConfiguration {
            ice_servers: vec![RTCIceServer {
                urls: vec![stun_adress.to_owned()],
                ..Default::default()
            }],
            ..Default::default()
        };

        // Create a new RTCPeerConnection
        let peer_connection = Arc::new(if let Ok(val) = api.new_peer_connection(config).await {
            val
        } else {
            return Err(Error::new(
                ErrorKind::Other,
                "Error creating peer connection",
            ));
        });

        Ok(Self { peer_connection })
    }
    /// Waits to recibe an sdp string offer to setting the pc remote description
    pub async fn set_sdp(&self) -> Result<(), Error> {
        println!("Paste the SDP offer from the remote peer");
        let line = must_read_stdin()?;
        let desc_data = decode(line.as_str())?;
        let offer = serde_json::from_str::<RTCSessionDescription>(&desc_data)?;
        // Set the remote SessionDescription
        if let Err(val) = self.peer_connection.set_remote_description(offer).await {
            return Err(Error::new(
                ErrorKind::Other,
                "Error setting remote description",
            ));
        };
        Ok(())
    }

    pub fn get_peer(&self) -> Arc<RTCPeerConnection>{
        self.peer_connection.clone()
    }
}

fn create_api() -> Result<API, Error> {
    let mut m = MediaEngine::default();
    //TODO: usar contantes o env
    if let Err(val) = m.register_codec(
        RTCRtpCodecParameters {
            capability: RTCRtpCodecCapability {
                mime_type: MIME_TYPE_OPUS.to_owned(),
                clock_rate: 48000,
                channels: 2,
                sdp_fmtp_line: "".to_owned(),
                rtcp_feedback: vec![],
            },
            payload_type: 111,
            ..Default::default()
        },
        RTPCodecType::Audio,
    ) {
        return Err(Error::new(ErrorKind::Other, "Error registering codec"));
    }

    let mut registry = Registry::new();

    // Use the default set of Interceptors
    if let Ok(val) = register_default_interceptors(registry, &mut m) {
        registry = val;
    } else {
        return Err(Error::new(
            ErrorKind::Other,
            "Error registering default interceptors",
        ));
    }

    // Create the API object with the MediaEngine
    let api = APIBuilder::new()
        .with_media_engine(m)
        .with_interceptor_registry(registry)
        .build();
    return Ok(api);
}


fn decode(s: &str) -> Result<String, Error> {
    let b = match BASE64_STANDARD.decode(s) {
        Ok(b) => b,
        Err(e) => return Err(Error::new(ErrorKind::Other, "Error decoding base64")),
    };

    //if COMPRESS {
    //    b = unzip(b)
    //}

    match String::from_utf8(b) {
        Ok(s) => return Ok(s),
        Err(e) => return Err(Error::new(ErrorKind::Other, "Error decoding utf8")),
    }
}

fn encode(b: &str) -> String {
    //if COMPRESS {
    //    b = zip(b)
    //}

    BASE64_STANDARD.encode(b)
}
