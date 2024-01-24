pub mod audio;
pub mod utils;
pub mod webrtcommunication;

#[tokio::main]
async fn main() -> Result<(), ()> {
    println!("Hola Mundo! 👋");

    Ok(())
}
