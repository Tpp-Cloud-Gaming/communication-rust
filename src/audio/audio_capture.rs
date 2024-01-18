use std::io::Error;
use std::sync::mpsc::Sender;

use cpal::{Device, SupportedStreamConfig, Stream, traits::{DeviceTrait, StreamTrait}, Sample, FromSample};

use crate::audio::audio_utils::search_device;

pub struct AudioCapture {
    device: Device,
    config: SupportedStreamConfig,
    stream: Option<Stream>,
    sender: Sender<Vec<f32>>,
}

impl AudioCapture {
    pub fn new(device_name: String, sender: Sender<Vec<f32>>) -> Result<Self, Error> {
        
        let device = search_device(device_name)?;
        log::info!("Device find: {}", device.name().unwrap());

        let config = match device.default_output_config() {
            Ok(config) => config,
            Err(_) => {
                return Err(Error::new(
                    std::io::ErrorKind::Other,
                    "Failed to get default device config",
                ))
            }
        };
        log::info!("Device Config: {:?}" , config);

        Ok(Self {
            device,
            config,
            stream: None,
            sender,
        })
    }

    pub fn start(&mut self) -> Result<Stream, Error> {

        let err_fn = move |err| {
            log::debug!("an error occurred on stream: {}", err);
        };

        let config_cpy = self.config.clone();
        let send_cpy = self.sender.clone();

        let stream = match self.config.sample_format() {
            cpal::SampleFormat::I8 => self
                .device
                .build_input_stream(
                    &config_cpy.into(),
                    move |data, _: &_| {
                        write_input_data::<i8, i8>(data, send_cpy.clone() )
                    },
                    err_fn,
                    None,
                )
                .unwrap(),
            cpal::SampleFormat::I16 => self
                .device
                .build_input_stream(
                    &config_cpy.into(),
                    move |data, _: &_| {
                        write_input_data::<i16, i16>(data, send_cpy.clone())
                    },
                    err_fn,
                    None,
                )
                .unwrap(),
            cpal::SampleFormat::I32 => self
                .device
                .build_input_stream(
                    &config_cpy.into(),
                    move |data, _: &_| {
                        write_input_data::<i32, i32>(data, send_cpy.clone())
                    },
                    err_fn,
                    None,
                )
                .unwrap(),
            cpal::SampleFormat::F32 => self
                .device
                .build_input_stream(
                    &config_cpy.into(),
                    move |data, _: &_| {
                        write_input_data::<f32, f32>(data, send_cpy.clone())
                    },
                    err_fn,
                    None,
                )
                .unwrap(),
            sample_format => {
                return Err(Error::new(
                    std::io::ErrorKind::Other,
                    format!("Unsupported sample format {:?}", sample_format),
                ));
            }
        };

        match stream.play(){
            Ok(_) => return  Ok(stream),
            Err(_) => return Err(Error::new(
                std::io::ErrorKind::Other,
                "Error playing stream",
            )),
        };

        
    }

    pub fn stop(&mut self) -> Result<(), Error> {
        match self.stream.take() {
            Some(stream) => drop(stream),
            None => {}
        };
        Ok(())
    }

}


fn write_input_data<T, U>(input: &[f32], sender: Sender<Vec<f32>>)
where
    T: Sample,
    U: Sample + hound::Sample + FromSample<T>,
{   
    /*if let Ok(mut guard) = encoder.lock() {

        match guard.encode_vec_float(input, 960){
            Ok(buffer) => {
                sender.send(buffer).unwrap();
            },
            Err(e) => {
                println!("Error: {:?}", e);
            }
        }
    }*/
    sender.send(input.to_vec()).unwrap();
    // let buffer = encoder
    //     .lock()
    //     .unwrap()
    //     .encode_vec_float(input, 960)
    //     .unwrap();
        
    // sender.send(buffer).unwrap();
}