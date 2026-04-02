use crate::android_inject::AndroidInjector;
use crate::config::{Config, TouchBackendKind};
use crate::engine::TouchCommand;
use crate::error::{PhantomError, Result};
use crate::inject::UinputDevice;
use std::collections::HashMap;

pub const MAX_CONCURRENT_TOUCHES: usize = 10;

#[derive(Debug, Default)]
pub struct SlotAllocator {
    logical_to_physical: HashMap<u8, u8>,
    physical_to_logical: [Option<u8>; MAX_CONCURRENT_TOUCHES],
}

impl SlotAllocator {
    pub fn physical_for(&self, logical_slot: u8) -> Option<u8> {
        self.logical_to_physical.get(&logical_slot).copied()
    }

    pub fn ensure_physical(&mut self, logical_slot: u8) -> Result<u8> {
        if let Some(slot) = self.physical_for(logical_slot) {
            return Ok(slot);
        }

        let Some((physical, owner)) = self
            .physical_to_logical
            .iter_mut()
            .enumerate()
            .find(|(_, owner)| owner.is_none())
        else {
            return Err(PhantomError::TouchBackend(format!(
                "too many concurrent touches: logical slot {} would exceed the {}-touch runtime limit",
                logical_slot, MAX_CONCURRENT_TOUCHES
            )));
        };

        *owner = Some(logical_slot);
        let physical = physical as u8;
        self.logical_to_physical.insert(logical_slot, physical);
        Ok(physical)
    }

    pub fn release(&mut self, logical_slot: u8) -> Option<u8> {
        let physical = self.logical_to_physical.remove(&logical_slot)?;
        self.physical_to_logical[physical as usize] = None;
        Some(physical)
    }

    pub fn active_count(&self) -> usize {
        self.logical_to_physical.len()
    }

    pub fn active_physical_slots(&self) -> Vec<u8> {
        self.physical_to_logical
            .iter()
            .enumerate()
            .filter_map(|(idx, owner)| owner.map(|_| idx as u8))
            .collect()
    }

    pub fn active_pairs(&self) -> Vec<(u8, u8)> {
        self.logical_to_physical
            .iter()
            .map(|(&logical, &physical)| (logical, physical))
            .collect()
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocator_reuses_existing_mapping() {
        let mut allocator = SlotAllocator::default();
        let first = allocator.ensure_physical(12).unwrap();
        let second = allocator.ensure_physical(12).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn allocator_releases_and_reuses_physical_slots() {
        let mut allocator = SlotAllocator::default();
        let first = allocator.ensure_physical(12).unwrap();
        let _ = allocator.ensure_physical(13).unwrap();
        assert_eq!(allocator.release(12), Some(first));
        let third = allocator.ensure_physical(14).unwrap();
        assert_eq!(third, first);
    }

    #[test]
    fn allocator_enforces_runtime_touch_limit() {
        let mut allocator = SlotAllocator::default();
        for logical in 0..MAX_CONCURRENT_TOUCHES as u8 {
            allocator.ensure_physical(logical).unwrap();
        }
        assert!(allocator.ensure_physical(99).is_err());
    }
}
