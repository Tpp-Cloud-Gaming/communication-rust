use crate::utils::common_utils::must_read_stdin;

use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use std::io::{Error, ErrorKind};
use std::sync::Arc;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::{MediaEngine, MIME_TYPE_H264, MIME_TYPE_OPUS};
use webrtc::api::{APIBuilder, API};
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtp_transceiver::rtp_codec::{
    RTCRtpCodecCapability, RTCRtpCodecParameters, RTPCodecType,
};

use crate::utils::webrtc_const::{
    AUDIO_CHANNELS, AUDIO_PAYLOAD_TYPE, AUDIO_SAMPLE_RATE, VIDEO_CHANNELS, VIDEO_PAYLOAD_TYPE,
    VIDEO_SAMPLE_RATE,
};
use crate::utils::webrtc_const::{TURN_ADRESS, TURN_PASS, TURN_USER};

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

        // Config SIN TURN SERVER
        // let config = RTCConfiguration {
        //     ice_servers: vec![RTCIceServer {
        //         urls: vec![stun_adress.to_owned()],
        //         ..Default::default()
        //     }],
        //     ..Default::default()
        // };

        //Config con TURN SERVER nuestro
        let config = RTCConfiguration {
            ice_servers: vec![
                RTCIceServer {
                    urls: vec![stun_adress.to_owned()],
                    ..Default::default()
                },
                RTCIceServer {
                    urls: vec![TURN_ADRESS.to_owned()],
                    username: TURN_USER.to_owned(),
                    credential: TURN_PASS.to_owned(),
                    credential_type:
                        webrtc::ice_transport::ice_credential_type::RTCIceCredentialType::Password,
                },
            ],
            ..Default::default()
        };

        //Config con TURN SERVER metered
        // let config = RTCConfiguration {
        //     ice_servers: vec![
        //         RTCIceServer {
        //             urls: vec![stun_adress.to_owned()],
        //             ..Default::default()
        //         },
        //         RTCIceServer {
        //             urls: vec!["turn:global.relay.metered.ca:80".to_owned()],
        //             username: "c746524136d0d233280283c2".to_owned(),
        //             credential: "KW+Xc4ju7DIPlrAX".to_owned(),
        //             credential_type:
        //                 webrtc::ice_transport::ice_credential_type::RTCIceCredentialType::Password,
        //         },
        //         RTCIceServer {
        //             urls: vec!["turn:global.relay.metered.ca:80?transport=tcp".to_owned()],
        //             username: "c746524136d0d233280283c2".to_owned(),
        //             credential: "KW+Xc4ju7DIPlrAX".to_owned(),
        //             credential_type:
        //                 webrtc::ice_transport::ice_credential_type::RTCIceCredentialType::Password,
        //         },
        //         RTCIceServer {
        //             urls: vec!["turn:global.relay.metered.ca:443".to_owned()],
        //             username: "c746524136d0d233280283c2".to_owned(),
        //             credential: "KW+Xc4ju7DIPlrAX".to_owned(),
        //             credential_type:
        //                 webrtc::ice_transport::ice_credential_type::RTCIceCredentialType::Password,
        //         },
        //         RTCIceServer {
        //             urls: vec!["turns:global.relay.metered.ca:443?transport=tcp".to_owned()],
        //             username: "c746524136d0d233280283c2".to_owned(),
        //             credential: "KW+Xc4ju7DIPlrAX".to_owned(),
        //             credential_type:
        //                 webrtc::ice_transport::ice_credential_type::RTCIceCredentialType::Password,
        //         },
        //     ],
        //     ..Default::default()
        // };
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
    pub async fn set_sdp(&self, sdp:String) -> Result<(), Error> {
        //let line = must_read_stdin()?;
        let desc_data = decode(sdp.as_str())?;
        let offer: RTCSessionDescription = serde_json::from_str::<RTCSessionDescription>(&desc_data)?;
        // Set the remote SessionDescription
        if self
            .peer_connection
            .set_remote_description(offer)
            .await
            .is_err()
        {
            return Err(Error::new(
                ErrorKind::Other,
                "Error setting remote description",
            ));
        };
        Ok(())
    }

    pub fn get_peer(&self) -> Arc<RTCPeerConnection> {
        self.peer_connection.clone()
    }
}

/// Creates the API object used for WebRTC communication.
///
/// Register codecs used for audio and video, and set up default interceptros.
///
/// # Returns
/// A Result containing the configured WebRTC API on success. Otherwise
/// error is returned
fn create_api() -> Result<API, Error> {
    let mut m = MediaEngine::default();
    if let Err(_val) = m.register_codec(
        RTCRtpCodecParameters {
            capability: RTCRtpCodecCapability {
                mime_type: MIME_TYPE_OPUS.to_owned(),
                clock_rate: AUDIO_SAMPLE_RATE,
                channels: AUDIO_CHANNELS,
                sdp_fmtp_line: "".to_owned(),
                rtcp_feedback: vec![],
            },
            payload_type: AUDIO_PAYLOAD_TYPE,
            ..Default::default()
        },
        RTPCodecType::Audio,
    ) {
        return Err(Error::new(ErrorKind::Other, "Error registering OPUS codec"));
    }

    if let Err(_val) = m.register_codec(
        RTCRtpCodecParameters {
            capability: RTCRtpCodecCapability {
                mime_type: MIME_TYPE_H264.to_owned(),
                clock_rate: VIDEO_SAMPLE_RATE,
                channels: VIDEO_CHANNELS,
                sdp_fmtp_line: "".to_owned(),
                rtcp_feedback: vec![],
            },
            payload_type: VIDEO_PAYLOAD_TYPE,
            ..Default::default()
        },
        RTPCodecType::Video,
    ) {
        return Err(Error::new(ErrorKind::Other, "Error registering H264 codec"));
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
    Ok(api)
}

/// Decode a base64 string
/// # Arguments
/// * `s` - &str that represents the base64 string
/// # Returns
/// * Result<String, Error> - The decoded string
fn decode(s: &str) -> Result<String, Error> {
    let b = match BASE64_STANDARD.decode(s) {
        Ok(b) => b,
        Err(_) => return Err(Error::new(ErrorKind::Other, "Error decoding base64")),
    };

    match String::from_utf8(b) {
        Ok(s) => Ok(s),
        Err(_) => Err(Error::new(ErrorKind::Other, "Error decoding utf8")),
    }
}

/// Encodes a string to base64
/// # Arguments
/// * `b` - The string to be encoded
/// # Returns
/// * Result<String, Error> - The encoded string
pub fn encode(b: &str) -> String {
    BASE64_STANDARD.encode(b)
}
