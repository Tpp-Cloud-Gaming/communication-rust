pub const ENCODE_BUFFER_SIZE: usize = 960;

pub const STREAM_TRACK_ID: &str = "webrtc-rs";
pub const STUN_ADRESS: &str = "stun:stun.l.google.com:19302";

// AUDIO
pub const AUDIO_SAMPLE_RATE: u32 = 48000;
pub const AUDIO_CHANNELS: u16 = 2;
pub const AUDIO_PAYLOAD_TYPE: u8 = 111;
pub const AUDIO_TRACK_ID: &str = "audio";

// VIDEO
pub const VIDEO_SAMPLE_RATE: u32 = 90000;
pub const VIDEO_PAYLOAD_TYPE: u8 = 96;
pub const VIDEO_CHANNELS: u16 = 2;
pub const VIDEO_TRACK_ID: &str = "video";

// Error Tracker parameters
//SENDER
pub const READ_TRACK_THRESHOLD: u32 = 500;
pub const READ_TRACK_LIMIT: u32 = 1000;
//RECEIVER
pub const SEND_TRACK_THRESHOLD: u32 = 500;
pub const SEND_TRACK_LIMIT: u32 = 1000;
