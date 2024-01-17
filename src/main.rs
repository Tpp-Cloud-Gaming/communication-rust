mod codecs;
mod utils;
mod webrtc;
use dotenv::dotenv;

#[tokio::main]
async fn main() -> Result<(), ()> {
    dotenv().ok();
    println!("Hola Mundo! 👋");
    Ok(())
}
