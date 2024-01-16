use std::sync::Arc;
use webrtc::peer_connection::RTCPeerConnection;
use base64::prelude::BASE64_STANDARD;
use base64::Engine;

pub struct Communication {
    peer_connection: Arc<RTCPeerConnection>,
}

// #[tokio::main]
// impl Communication {
//     #[tokio::main]
//     async pub fn new( stun_adress: String ){

//         let mut m = MediaEngine::default();

//         m.register_codec(
//             RTCRtpCodecParameters {
//                 capability: RTCRtpCodecCapability {
//                     mime_type: MIME_TYPE_OPUS.to_owned(),
//                     clock_rate: 48000,
//                     channels: 2,
//                     sdp_fmtp_line: "".to_owned(),
//                     rtcp_feedback: vec![],
//                 },
//                 payload_type: 111,
//                 ..Default::default()
//             },
//             RTPCodecType::Audio,
//         )?;
        
//         let mut registry = Registry::new();

//         // Use the default set of Interceptors
//         registry = register_default_interceptors(registry, &mut m)?;

//         // Create the API object with the MediaEngine
//         let api = APIBuilder::new()
//             .with_media_engine(m)
//             .with_interceptor_registry(registry)
//             .build();

//         let config = RTCConfiguration {
//             ice_servers: vec![RTCIceServer {
//                 urls: vec![stun_adress.to_owned()],
//                 ..Default::default()
//             }],
//             ..Default::default()
//         };

//         // Create a new RTCPeerConnection
//         let peer_connection = Arc::new(api.new_peer_connection(config).await?);

//     }
// }


enum SDPDecodeError {
    Base64Error,
    FromUtf8Error,
}

fn decode(s: &str) -> Result<String, SDPDecodeError> {
    let b = match BASE64_STANDARD.decode(s) {
        Ok(b) => b,
        Err(e) => return Err(SDPDecodeError::Base64Error),
    };

    //if COMPRESS {
    //    b = unzip(b)
    //}

    match String::from_utf8(b) {
        Ok(s) => return Ok(s),
        Err(e) => return Err(SDPDecodeError::FromUtf8Error),
    }
}

fn encode(b: &str) -> String {
    //if COMPRESS {
    //    b = zip(b)
    //}

    BASE64_STANDARD.encode(b)
}