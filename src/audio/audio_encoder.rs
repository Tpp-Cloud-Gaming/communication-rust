use opus::Encoder;

use std::io::Error;

use crate::utils::webrtc_const::ENCODE_BUFFER_SIZE;

use crate::utils::webrtc_const::AUDIO_SAMPLE_RATE;

pub struct AudioEncoder {
    encoder: Encoder,
}

impl AudioEncoder {
    /// Returns new instance of audio encoder.
    pub fn new() -> Result<Self, Error> {
        let encoder =
            opus::Encoder::new(AUDIO_SAMPLE_RATE, opus::Channels::Stereo, opus::Application::Voip)
                .map_err(|e| Error::new(std::io::ErrorKind::Other, e))?;

        Ok(Self { encoder })
    }

    /// Returns an encoded opus sample as a Vec<f32>
    /// # Arguments
    ///
    /// * `input` - Vec<u8> that represents an audio sample
    pub fn encode(&mut self, data: Vec<f32>) -> Result<Vec<u8>, opus::Error> {
        match self.encoder.encode_vec_float(&data, ENCODE_BUFFER_SIZE) {
            Ok(buffer) => Ok(buffer),
            Err(e) => Err(e),
        }
    }
}
