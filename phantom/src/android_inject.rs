use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;
use std::time::Duration;

use crate::config::Config;
use crate::engine::TouchCommand;
use crate::error::{PhantomError, Result};
use crate::touch::TouchDevice;
use crate::waydroid;

const MAX_SLOTS: usize = 10;

const CMD_TOUCH_DOWN: u8 = 0x00;
const CMD_TOUCH_MOVE: u8 = 0x01;
const CMD_TOUCH_UP: u8 = 0x02;
const CMD_TOUCH_CANCEL: u8 = 0x03;
const CMD_PING: u8 = 0x7f;
const AUTO_LAUNCH_CONNECT_TIMEOUT: Duration = Duration::from_secs(180);

pub struct AndroidInjector {
    stream: TcpStream,
    endpoint: String,
    screen_width: i32,
    screen_height: i32,
    active_slots: [bool; MAX_SLOTS],
    active_touches: usize,
}

impl AndroidInjector {
    pub fn from_config(config: &Config, screen_width: u32, screen_height: u32) -> Result<Self> {
        let host = waydroid::android_server_host(config)?;
        let port = waydroid::android_server_port(config);

        match Self::connect(&host, port, screen_width, screen_height) {
            Ok(mut injector) => {
                injector.ping()?;
                Ok(injector)
            }
            Err(initial_err) => {
                if !config.android.auto_launch {
                    return Err(initial_err);
                }

                tracing::info!(
                    host = host,
                    port = port,
                    error = %initial_err,
                    "android touch server unavailable, attempting auto-launch"
                );
                waydroid::ensure_android_server(config)?;

                let mut injector = match Self::connect_with_retry(
                    &host,
                    port,
                    screen_width,
                    screen_height,
                    AUTO_LAUNCH_CONNECT_TIMEOUT,
                ) {
                    Ok(injector) => injector,
                    Err(err) => {
                        if let Some(log) = waydroid::android_server_log_excerpt(config) {
                            return Err(PhantomError::TouchBackend(format!(
                                "{}\nandroid server log:\n{}",
                                err, log
                            )));
                        }
                        return Err(err);
                    }
                };
                injector.ping()?;
                Ok(injector)
            }
        }
    }

    pub fn connect(host: &str, port: u16, screen_width: u32, screen_height: u32) -> Result<Self> {
        let endpoint = format!("{}:{}", host, port);
        let stream = TcpStream::connect((host, port)).map_err(|e| {
            PhantomError::TouchBackend(format!(
                "cannot connect to android touch server {}: {}",
                endpoint, e
            ))
        })?;

        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .map_err(PhantomError::Io)?;
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .map_err(PhantomError::Io)?;
        stream.set_nodelay(true).map_err(PhantomError::Io)?;

        tracing::info!(endpoint = endpoint, "connected to android touch server");

        Ok(Self {
            stream,
            endpoint,
            screen_width: screen_width as i32,
            screen_height: screen_height as i32,
            active_slots: [false; MAX_SLOTS],
            active_touches: 0,
        })
    }

    fn connect_with_retry(
        host: &str,
        port: u16,
        screen_width: u32,
        screen_height: u32,
        timeout: Duration,
    ) -> Result<Self> {
        let deadline = std::time::Instant::now() + timeout;
        let mut last_err = None;

        while std::time::Instant::now() < deadline {
            match Self::connect(host, port, screen_width, screen_height) {
                Ok(injector) => return Ok(injector),
                Err(err) => {
                    last_err = Some(err);
                    thread::sleep(Duration::from_millis(100));
                }
            }
        }

        Err(last_err.unwrap_or_else(|| {
            PhantomError::TouchBackend(format!(
                "android touch server did not accept TCP connections at {}:{} within {:?}",
                host, port, timeout
            ))
        }))
    }

    pub fn ping(&mut self) -> Result<()> {
        self.write_frame(&[CMD_PING])?;
        let mut reply = [0u8; 1];
        self.stream.read_exact(&mut reply).map_err(|e| {
            PhantomError::TouchBackend(format!(
                "android touch server ping failed on {}: {}",
                self.endpoint, e
            ))
        })?;

        if reply[0] != CMD_PING {
            return Err(PhantomError::TouchBackend(format!(
                "android touch server {} replied with invalid ping byte {:#x}",
                self.endpoint, reply[0]
            )));
        }

        Ok(())
    }

    fn write_frame(&mut self, frame: &[u8]) -> Result<()> {
        self.stream.write_all(frame).map_err(|e| {
            PhantomError::TouchBackend(format!(
                "failed writing to android touch server {}: {}",
                self.endpoint, e
            ))
        })?;
        self.stream.flush().map_err(|e| {
            PhantomError::TouchBackend(format!(
                "failed flushing android touch server {}: {}",
                self.endpoint, e
            ))
        })
    }

    fn write_position_frame(&mut self, kind: u8, slot: u8, x: i32, y: i32) -> Result<()> {
        let frame = Self::position_frame(kind, slot, x, y);
        self.write_frame(&frame)
    }

    fn write_slot_frame(&mut self, kind: u8, slot: u8) -> Result<()> {
        let frame = Self::slot_frame(kind, slot);
        self.write_frame(&frame)
    }

    fn touch_down_inner(&mut self, slot: u8, x: f64, y: f64) -> Result<()> {
        let slot_idx = self.slot_index(slot)?;
        if self.active_slots[slot_idx] {
            tracing::debug!(
                "android slot {} already active, treating touch_down as touch_move",
                slot
            );
            return self.touch_move_inner(slot, x, y);
        }

        let (px, py) = self.scale_coords(x, y);
        self.write_position_frame(CMD_TOUCH_DOWN, slot, px, py)?;
        self.active_slots[slot_idx] = true;
        self.active_touches += 1;
        Ok(())
    }

    fn touch_move_inner(&mut self, slot: u8, x: f64, y: f64) -> Result<()> {
        let slot_idx = self.slot_index(slot)?;
        if !self.active_slots[slot_idx] {
            return Err(PhantomError::Profile(format!(
                "touch_move on inactive slot {}",
                slot
            )));
        }

        let (px, py) = self.scale_coords(x, y);
        self.write_position_frame(CMD_TOUCH_MOVE, slot, px, py)
    }

    fn touch_up_inner(&mut self, slot: u8, cancel: bool) -> Result<()> {
        let slot_idx = self.slot_index(slot)?;
        if !self.active_slots[slot_idx] {
            return Ok(());
        }

        self.write_slot_frame(
            if cancel {
                CMD_TOUCH_CANCEL
            } else {
                CMD_TOUCH_UP
            },
            slot,
        )?;
        self.active_slots[slot_idx] = false;
        self.active_touches = self.active_touches.saturating_sub(1);
        Ok(())
    }

    fn scale_coords(&self, x: f64, y: f64) -> (i32, i32) {
        let px = ((x.clamp(0.0, 1.0)) * (self.screen_width as f64)) as i32;
        let py = ((y.clamp(0.0, 1.0)) * (self.screen_height as f64)) as i32;

        (
            px.clamp(0, self.screen_width.saturating_sub(1)),
            py.clamp(0, self.screen_height.saturating_sub(1)),
        )
    }

    fn slot_index(&self, slot: u8) -> Result<usize> {
        let idx = slot as usize;
        if idx >= self.active_slots.len() {
            return Err(PhantomError::Profile(format!(
                "slot {} out of range 0-{}",
                slot,
                self.active_slots.len().saturating_sub(1)
            )));
        }
        Ok(idx)
    }

    fn active_slot_ids(&self) -> Vec<u8> {
        self.active_slots
            .iter()
            .enumerate()
            .filter_map(|(idx, active)| active.then_some(idx as u8))
            .collect()
    }

    fn position_frame(kind: u8, slot: u8, x: i32, y: i32) -> [u8; 10] {
        let mut frame = [0u8; 10];
        frame[0] = kind;
        frame[1] = slot;
        frame[2..6].copy_from_slice(&x.to_le_bytes());
        frame[6..10].copy_from_slice(&y.to_le_bytes());
        frame
    }

    fn slot_frame(kind: u8, slot: u8) -> [u8; 2] {
        [kind, slot]
    }
}

impl TouchDevice for AndroidInjector {
    fn backend_name(&self) -> &'static str {
        "android_socket"
    }

    fn apply_commands(&mut self, cmds: &[TouchCommand]) -> Result<()> {
        if cmds.is_empty() {
            return Ok(());
        }

        tracing::trace!(count = cmds.len(), ?cmds, "injecting android touch batch");

        for cmd in cmds {
            match cmd {
                TouchCommand::TouchDown { slot, x, y } => self.touch_down_inner(*slot, *x, *y)?,
                TouchCommand::TouchMove { slot, x, y } => self.touch_move_inner(*slot, *x, *y)?,
                TouchCommand::TouchUp { slot } => self.touch_up_inner(*slot, false)?,
            }
        }

        tracing::trace!(
            active_touches = self.active_touches,
            active_slots = ?self.active_slot_ids(),
            "android touch batch applied"
        );
        Ok(())
    }

    fn release_all(&mut self) -> Result<()> {
        for slot in 0..MAX_SLOTS as u8 {
            if self.active_slots[slot as usize] {
                self.touch_up_inner(slot, true)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_frame_uses_little_endian_coordinates() {
        let frame = AndroidInjector::position_frame(CMD_TOUCH_DOWN, 2, 960, 270);

        assert_eq!(frame[0], CMD_TOUCH_DOWN);
        assert_eq!(frame[1], 2);
        assert_eq!(i32::from_le_bytes(frame[2..6].try_into().unwrap()), 960);
        assert_eq!(i32::from_le_bytes(frame[6..10].try_into().unwrap()), 270);
    }

    #[test]
    fn slot_frame_is_two_bytes() {
        assert_eq!(
            AndroidInjector::slot_frame(CMD_TOUCH_UP, 4),
            [CMD_TOUCH_UP, 4]
        );
    }
}
