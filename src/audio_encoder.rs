use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, FromSample, Sample, Stream, SupportedStreamConfig};
use opus::Encoder;
use std::io::Error;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

pub struct AudioEncoder {
    device: Device,
    encoder: Arc<Mutex<Encoder>>,
    config: SupportedStreamConfig,
    pub stream: Option<Stream>,
    sender: Sender<Vec<u8>>,
}

impl AudioEncoder {
    pub fn new(device_name: String, sender: Sender<Vec<u8>>) -> Result<Self, Error> {
        let device = Self::search_device(device_name)?;

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

        log::info!("Device config: {}", device.name().unwrap());

        let sample_rate: u32 = std::env::var("SAMPLE_RATE")
            .expect("SAMPLE_RATE env not found.")
            .parse()
            .expect("Failed to parse SAMPLE_RATE as u32");

        let mut encoder =
            opus::Encoder::new(sample_rate, opus::Channels::Stereo, opus::Application::Voip)
                .unwrap();

        Ok(Self {
            device: device,
            encoder: Arc::new(Mutex::new(encoder)),
            config: config,
            stream: None,
            sender: sender,
        })
    }

    pub fn start(&mut self) -> Result<Stream, Error> {
        let err_fn = move |err| {
            log::debug!("an error occurred on stream: {}", err);
        };

        let mut config_cpy = self.config.clone();

        let enc_cpy = self.encoder.clone();
        let send_cpy = self.sender.clone();

        let stream = match self.config.sample_format() {
            cpal::SampleFormat::I8 => self
                .device
                .build_input_stream(
                    &config_cpy.into(),
                    move |data, _: &_| {
                        write_input_data::<i8, i8>(data, &mut enc_cpy.clone(), send_cpy.clone())
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
                        write_input_data::<i16, i16>(data, &mut enc_cpy.clone(), send_cpy.clone())
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
                        write_input_data::<i32, i32>(data, &mut enc_cpy.clone(), send_cpy.clone())
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
                        write_input_data::<f32, f32>(data, &mut enc_cpy.clone(), send_cpy.clone())
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
        Ok(stream)
    }

    pub fn stop(&mut self) -> Result<(), Error> {
        match self.stream.take() {
            Some(stream) => drop(stream),
            None => {}
        };
        Ok(())
    }

    fn search_device(name: String) -> Result<Device, Error> {
        let host = cpal::default_host();

        // Set up the input device and stream with the default input config.
        let mut device = match host.default_input_device() {
            Some(device) => device,
            None => {
                return Err(Error::new(
                    std::io::ErrorKind::Other,
                    "Failed to get default input device",
                ))
            }
        };

        let output_devices = match host.output_devices() {
            Ok(devices) => devices,
            Err(_) => {
                return Err(Error::new(
                    std::io::ErrorKind::Other,
                    "Failed to get output devices",
                ))
            }
        };

        for actual_device in output_devices {
            let actual_name = match actual_device.name() {
                Ok(n) => n,
                Err(_) => continue,
            };

            if actual_name.contains(&name) {
                device = actual_device;
            }
        }
        return Ok(device);
    }
}

fn write_input_data<T, U>(input: &[f32], encoder: &mut Arc<Mutex<Encoder>>, sender: Sender<Vec<u8>>)
where
    T: Sample,
    U: Sample + hound::Sample + FromSample<T>,
{   


    if let Ok(mut guard) = encoder.lock() {

        match guard.encode_vec_float(input, 960){
            Ok(buffer) => {
                sender.send(buffer).unwrap();
            },
            Err(e) => {
                println!("Error: {:?}", e);
            }
        }
    }
    // let buffer = encoder
    //     .lock()
    //     .unwrap()
    //     .encode_vec_float(input, 960)
    //     .unwrap();
        
    // sender.send(buffer).unwrap();
}
