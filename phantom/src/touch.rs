use crate::android_inject::AndroidInjector;
use crate::config::{Config, TouchBackendKind};
use crate::engine::TouchCommand;
use crate::error::Result;
use crate::inject::UinputDevice;

pub trait TouchDevice: Send {
    fn backend_name(&self) -> &'static str;
    fn apply_commands(&mut self, cmds: &[TouchCommand]) -> Result<()>;
    fn release_all(&mut self) -> Result<()>;
}

pub fn create_touch_device(
    config: &Config,
    screen_width: u32,
    screen_height: u32,
) -> Result<Box<dyn TouchDevice>> {
    let device: Box<dyn TouchDevice> = match config.touch_backend {
        TouchBackendKind::Uinput => Box::new(UinputDevice::new(screen_width, screen_height)?),
        TouchBackendKind::AndroidSocket => Box::new(AndroidInjector::from_config(
            config,
            screen_width,
            screen_height,
        )?),
    };

    tracing::info!(backend = device.backend_name(), "touch backend ready");
    Ok(device)
}
