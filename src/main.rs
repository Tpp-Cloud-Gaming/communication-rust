
mod webrtc;
mod codecs;
use dotenv::dotenv;

#[tokio::main]
async fn main() -> Result<(),()> {
    dotenv().ok();
    println!("Hola Mundo! 👋");
    Ok(())
}