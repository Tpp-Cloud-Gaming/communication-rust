pub mod utilss;
pub mod codecs;
pub mod webrtcommunication;
use dotenv::dotenv;

#[tokio::main]
async fn main() -> Result<(), ()> {
    dotenv().ok();
    println!("Hola Mundo! 👋");
    Ok(())
}
