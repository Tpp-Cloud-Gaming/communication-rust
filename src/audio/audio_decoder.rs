use opus::Decoder;
use std::io::Error;

pub struct AudioDecoder{
    decoder: Decoder,
}

impl AudioDecoder {
    pub fn new() -> Result<Self, Error> {
        let sample_rate: u32 = std::env::var("SAMPLE_RATE")
            .expect("SAMPLE_RATE env not found.")
            .parse()
            .expect("Failed to parse SAMPLE_RATE as u32");
        let decoder = opus::Decoder::new(sample_rate, opus::Channels::Stereo).unwrap();
        Ok(Self { decoder: decoder})

    }
    
    pub fn decode(&mut self, input: Vec<u8>) -> Result<Vec<f32>, Error> {
        let mut data = vec![0.0; 960];

        match self.decoder.decode_float(&input, &mut data[..], false) {
            Ok(_) => {}
            Err(e) => {
                println!("Error: {:?}", e);
            }
        }
        Ok(data)
        
    }
}