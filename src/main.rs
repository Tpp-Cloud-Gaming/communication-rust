pub mod utils;
pub mod audio;
pub mod webrtcommunication;
use dotenv::dotenv;

use crate::audio::audio_encoder::AudioEncoder;

#[tokio::main]
async fn main() -> Result<(), ()> {
    dotenv().ok();
    println!("Hola Mundo! 👋");

    Ok(())


}
