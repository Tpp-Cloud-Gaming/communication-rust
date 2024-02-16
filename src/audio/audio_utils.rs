use std::io::Error;

use cpal::{
    traits::{DeviceTrait, HostTrait},
    Device,
};

/// Returns the audio device that matches the name or default if none
///
/// # Arguments
///
/// * `name` - An optional string that represents the device name
pub fn search_device(name: Option<String>) -> Result<Device, Error> {
    let host = cpal::default_host();

    if let Some(name) = name {
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
                return Ok(actual_device);
            }
        }
    }

    match host.default_output_device() {
        Some(default_output_device) => Ok(default_output_device),
        None => Err(Error::new(
            std::io::ErrorKind::Other,
            "Failed to get default output device",
        )),
    }
}
