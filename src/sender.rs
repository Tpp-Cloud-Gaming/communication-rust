use crate::codecs;
use crate::webrtc;
use codecs::audio_decoder::AudioDecoder;
use codecs::audio_encoder::AudioEncoder;

#[tokio::main]
async fn main() -> Result<(),()> {
    dotenv().ok();
    println!("Hola Mundo soy el Sender! 👋");
    Ok(())
}