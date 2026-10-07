//! Platformy bez WASAPI: każde otwarcie kończy się `AudioError::Unsupported` (testy używają atrapy).

use voice_audio_contract::{
    AudioDevice, AudioError, AudioIo, DeviceEvent, DeviceId, InputStream, OutputStream,
    StreamConfig,
};

/// `AudioIo` dla systemów innych niż Windows.
#[derive(Debug, Default, Clone, Copy)]
pub struct UnsupportedAudio;

fn unsupported() -> AudioError {
    AudioError::Unsupported(format!(
        "{} (wymagany Windows/WASAPI)",
        std::env::consts::OS
    ))
}

impl AudioIo for UnsupportedAudio {
    fn devices(&self) -> Result<Vec<AudioDevice>, AudioError> {
        Err(unsupported())
    }

    fn poll_device_events(&self) -> Vec<DeviceEvent> {
        Vec::new()
    }

    fn open_input(
        &self,
        _device: Option<&DeviceId>,
        _config: &StreamConfig,
    ) -> Result<Box<dyn InputStream>, AudioError> {
        Err(unsupported())
    }

    fn open_output(
        &self,
        _device: Option<&DeviceId>,
        _config: &StreamConfig,
    ) -> Result<Box<dyn OutputStream>, AudioError> {
        Err(unsupported())
    }

    fn open_loopback(
        &self,
        _device: Option<&DeviceId>,
        _config: &StreamConfig,
    ) -> Result<Box<dyn InputStream>, AudioError> {
        Err(unsupported())
    }
}
