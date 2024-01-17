use crate::codecs;
use crate::webrtc;
use codecs::audio_decoder::AudioDecoder;
use codecs::audio_encoder::AudioEncoder;

#[tokio::main]
async fn main() -> Result<(), ()> {
    dotenv().ok();
    println!("Arranca el Sender! 👋");

    let comunication = Communication::new("stun:stun.l.google.com:19302".to_owned()).await?;

    comunication.set_sdp().await?;
    Ok(())
}
