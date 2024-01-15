use audio::audio_decoder::AudioDecoder;
use audio::audio_encoder::AudioEncoder;
use cpal::traits::StreamTrait;
use dotenv::dotenv;
use std::io::Error;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Error> {
    dotenv().ok(); // This line loads the environment variables from the ".env" file.

    let (tx, rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = mpsc::channel();

    let mut encoder = AudioEncoder::new(
        "Microphone Array)".to_string(),
        tx,
    )?;

    let stream = encoder.start().unwrap();
    stream.play().unwrap();

    // let mut decoder = AudioDecoder::new()?;

    // let mut counter = 0;

    // while (counter < 10000) {
    //     let data = rx.recv().unwrap();

    //     let decode_data = decoder.decode(data).unwrap();
    //     println!("{:?}", counter);
    //     counter += 1;
    // }

    thread::sleep(Duration::from_secs(5));

    drop(stream);
    //encoder.stop()?;
    Ok(())
}
