use opus::Decoder;
use std::io::Error;

use crate::utils::webrtc_const::{AUDIO_SAMPLE_RATE, ENCODE_BUFFER_SIZE};
/// Decodes opus samples.
pub struct AudioDecoder {
    /// Used to decode opus samples
    decoder: Decoder,
}

impl AudioDecoder {
    /// Returns a AudioDecoder
    pub fn new() -> Result<Self, Error> {
        let decoder = match opus::Decoder::new(AUDIO_SAMPLE_RATE, opus::Channels::Stereo) {
            Ok(decoder) => decoder,
            Err(_) => {
                return Err(Error::new(
                    std::io::ErrorKind::Other,
                    "Error creating opus decoder".to_string(),
                ))
            }
        };
        Ok(Self { decoder })
    }

    /// Returns a decoded opus sample as a Vec<f32>
    ///
    /// # Arguments
    ///
    /// * `input` - Vec<u8> that represents a opus sample
    pub fn decode(&mut self, input: Vec<u8>) -> Result<Vec<f32>, Error> {
        let mut data = vec![0.0; ENCODE_BUFFER_SIZE];

        match self.decoder.decode_float(&input, &mut data[..], false) {
            Ok(_) => {}
            Err(e) => {
                log::debug!("Error decoding from opus: {:?}", e);
            }
        }
        Ok(data)
    }
}
