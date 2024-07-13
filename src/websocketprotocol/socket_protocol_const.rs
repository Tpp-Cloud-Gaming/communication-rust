pub const SOCKET_URL: &str = "wss://cloud-gaming-server.onrender.com";

//SOCKET SEND MESSAGES
pub const INIT_OFFERER_MSG: &str = "initOfferer";
pub const OFFERER_SDP_MSG: &str = "offererSdp";
pub const INIT_CLIENT_MSG: &str = "initClient";
pub const CLIENT_SDP_MSG: &str = "clientSdp";
pub const START_SESSION_MSG: &str = "startSession";
pub const FORCE_STOP_SESSION_MSG: &str = "forceStopSession";

//SOCKET RECEIVE MESSAGES
pub const SDP_REQUEST_FROM_MSG: &str = "sdpRequestFrom";
pub const SDP_CLIENT_MSG: &str = "sdpClient";
pub const SDP_OFFERER_MSG: &str = "sdpOfferer";
pub const NOTIF_END_SESSION_MSG: &str = "notifEndSession";
