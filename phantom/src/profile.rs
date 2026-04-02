use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::error::{PhantomError, Result};
use crate::input::Key;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub version: u32,
    pub screen: Option<ScreenOverride>,
    #[serde(default = "default_sensitivity")]
    pub global_sensitivity: f64,
    pub nodes: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScreenOverride {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LayerMode {
    #[default]
    Hold,
    Toggle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MouseCameraActivationMode {
    #[default]
    AlwaysOn,
    WhileHeld,
    Toggle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum JoystickMode {
    #[default]
    Fixed,
    Floating,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Node {
    Tap {
        id: String,
        #[serde(default = "default_layer", skip_serializing_if = "is_default_layer")]
        layer: String,
        slot: u8,
        pos: RelPos,
        key: String,
    },
    HoldTap {
        id: String,
        #[serde(default = "default_layer", skip_serializing_if = "is_default_layer")]
        layer: String,
        slot: u8,
        pos: RelPos,
        key: String,
    },
    ToggleTap {
        id: String,
        #[serde(default = "default_layer", skip_serializing_if = "is_default_layer")]
        layer: String,
        slot: u8,
        pos: RelPos,
        key: String,
    },
    Joystick {
        id: String,
        #[serde(default = "default_layer", skip_serializing_if = "is_default_layer")]
        layer: String,
        slot: u8,
        pos: RelPos,
        radius: f64,
        #[serde(default, skip_serializing_if = "is_default_joystick_mode")]
        mode: JoystickMode,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        region: Option<Region>,
        keys: JoystickKeys,
    },
    Drag {
        id: String,
        #[serde(default = "default_layer", skip_serializing_if = "is_default_layer")]
        layer: String,
        slot: u8,
        start: RelPos,
        end: RelPos,
        key: String,
        duration_ms: u64,
    },
    MouseCamera {
        id: String,
        #[serde(default = "default_layer", skip_serializing_if = "is_default_layer")]
        layer: String,
        slot: u8,
        region: Region,
        sensitivity: f64,
        #[serde(
            default,
            skip_serializing_if = "is_default_mouse_camera_activation_mode"
        )]
        activation_mode: MouseCameraActivationMode,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        activation_key: Option<String>,
        #[serde(default)]
        invert_y: bool,
    },
    RepeatTap {
        id: String,
        #[serde(default = "default_layer", skip_serializing_if = "is_default_layer")]
        layer: String,
        slot: u8,
        pos: RelPos,
        key: String,
        interval_ms: u64,
    },
    Macro {
        id: String,
        #[serde(default = "default_layer", skip_serializing_if = "is_default_layer")]
        layer: String,
        key: String,
        sequence: Vec<MacroStep>,
    },
    LayerShift {
        id: String,
        key: String,
        layer_name: String,
        #[serde(default)]
        mode: LayerMode,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RelPos {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JoystickKeys {
    pub up: String,
    pub down: String,
    pub left: String,
    pub right: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Region {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MacroStep {
    pub action: MacroAction,
    #[serde(default)]
    pub pos: Option<RelPos>,
    pub slot: u8,
    #[serde(default)]
    pub delay_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MacroAction {
    Down,
    Up,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileAudit {
    pub profile_name: String,
    pub screen_width: u32,
    pub screen_height: u32,
    pub total_nodes: usize,
    pub touch_entries: Vec<SlotAuditEntry>,
    pub auxiliary_entries: Vec<AuxiliaryAuditEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotAuditEntry {
    pub slot: u8,
    pub node_id: String,
    pub node_type: &'static str,
    pub layer: String,
    pub bindings: Vec<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuxiliaryAuditEntry {
    pub node_id: String,
    pub node_type: &'static str,
    pub layer: String,
    pub bindings: Vec<String>,
    pub detail: Option<String>,
}

fn default_sensitivity() -> f64 {
    1.0
}

fn default_layer() -> String {
    String::new()
}

fn is_default_layer(layer: &str) -> bool {
    layer.trim().is_empty()
}

fn is_default_mouse_camera_activation_mode(mode: &MouseCameraActivationMode) -> bool {
    matches!(mode, MouseCameraActivationMode::AlwaysOn)
}

fn is_default_joystick_mode(mode: &JoystickMode) -> bool {
    matches!(mode, JoystickMode::Fixed)
}

impl Node {
    pub fn kind(&self) -> &'static str {
        match self {
            Node::Tap { .. } => "tap",
            Node::HoldTap { .. } => "hold_tap",
            Node::ToggleTap { .. } => "toggle_tap",
            Node::Joystick { .. } => "joystick",
            Node::Drag { .. } => "drag",
            Node::MouseCamera { .. } => "mouse_camera",
            Node::RepeatTap { .. } => "repeat_tap",
            Node::Macro { .. } => "macro",
            Node::LayerShift { .. } => "layer_shift",
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Node::Tap { id, .. }
            | Node::HoldTap { id, .. }
            | Node::ToggleTap { id, .. }
            | Node::Joystick { id, .. }
            | Node::Drag { id, .. }
            | Node::MouseCamera { id, .. }
            | Node::RepeatTap { id, .. }
            | Node::Macro { id, .. }
            | Node::LayerShift { id, .. } => id,
        }
    }

    pub fn layer(&self) -> &str {
        match self {
            Node::Tap { layer, .. }
            | Node::HoldTap { layer, .. }
            | Node::ToggleTap { layer, .. }
            | Node::Joystick { layer, .. }
            | Node::Drag { layer, .. }
            | Node::MouseCamera { layer, .. }
            | Node::RepeatTap { layer, .. }
            | Node::Macro { layer, .. } => layer.as_str(),
            Node::LayerShift { .. } => "",
        }
    }

    pub fn slot(&self) -> Option<u8> {
        match self {
            Node::Tap { slot, .. }
            | Node::HoldTap { slot, .. }
            | Node::ToggleTap { slot, .. }
            | Node::Joystick { slot, .. }
            | Node::Drag { slot, .. }
            | Node::MouseCamera { slot, .. }
            | Node::RepeatTap { slot, .. } => Some(*slot),
            Node::Macro { .. } | Node::LayerShift { .. } => None,
        }
    }

    pub fn bound_keys(&self) -> Vec<&str> {
        match self {
            Node::Tap { key, .. }
            | Node::HoldTap { key, .. }
            | Node::ToggleTap { key, .. }
            | Node::RepeatTap { key, .. }
            | Node::Drag { key, .. }
            | Node::Macro { key, .. }
            | Node::LayerShift { key, .. } => vec![key.as_str()],
            Node::Joystick { keys, .. } => vec![
                keys.up.as_str(),
                keys.down.as_str(),
                keys.left.as_str(),
                keys.right.as_str(),
            ],
            Node::MouseCamera { activation_key, .. } => {
                activation_key.as_deref().into_iter().collect()
            }
        }
    }

    pub fn primary_binding(&self) -> Option<&str> {
        match self {
            Node::Tap { key, .. }
            | Node::HoldTap { key, .. }
            | Node::ToggleTap { key, .. }
            | Node::RepeatTap { key, .. }
            | Node::Drag { key, .. }
            | Node::Macro { key, .. }
            | Node::LayerShift { key, .. } => Some(key),
            Node::Joystick { .. } => None,
            Node::MouseCamera { activation_key, .. } => activation_key.as_deref(),
        }
    }

    pub fn audit_bindings(&self) -> Vec<String> {
        match self {
            Node::Tap { key, .. }
            | Node::HoldTap { key, .. }
            | Node::ToggleTap { key, .. }
            | Node::RepeatTap { key, .. }
            | Node::Drag { key, .. }
            | Node::Macro { key, .. }
            | Node::LayerShift { key, .. } => vec![key.clone()],
            Node::Joystick { keys, .. } => vec![
                keys.up.clone(),
                keys.down.clone(),
                keys.left.clone(),
                keys.right.clone(),
            ],
            Node::MouseCamera { activation_key, .. } => {
                let mut bindings = vec!["MouseMove".into()];
                if let Some(key) = activation_key {
                    bindings.push(key.clone());
                }
                bindings
            }
        }
    }

    pub fn audit_detail(&self) -> Option<String> {
        match self {
            Node::Joystick {
                radius,
                mode,
                region,
                ..
            } => Some(match (mode, region) {
                (JoystickMode::Fixed, _) => format!("mode=fixed radius={:.3}", radius),
                (JoystickMode::Floating, Some(region)) => format!(
                    "mode=floating radius={:.3} region=({:.3},{:.3},{:.3},{:.3})",
                    radius, region.x, region.y, region.w, region.h
                ),
                (JoystickMode::Floating, None) => {
                    format!("mode=floating radius={:.3} region=<missing>", radius)
                }
            }),
            Node::Drag {
                start,
                end,
                duration_ms,
                ..
            } => Some(format!(
                "start=({:.3},{:.3}) end=({:.3},{:.3}) duration_ms={}",
                start.x, start.y, end.x, end.y, duration_ms
            )),
            Node::MouseCamera {
                region,
                sensitivity,
                activation_mode,
                activation_key,
                invert_y,
                ..
            } => Some(format!(
                "region=({:.3},{:.3},{:.3},{:.3}) sensitivity={:.3} mode={} key={} invert_y={}",
                region.x,
                region.y,
                region.w,
                region.h,
                sensitivity,
                match activation_mode {
                    MouseCameraActivationMode::AlwaysOn => "always_on",
                    MouseCameraActivationMode::WhileHeld => "while_held",
                    MouseCameraActivationMode::Toggle => "toggle",
                },
                activation_key.as_deref().unwrap_or("-"),
                invert_y
            )),
            Node::Macro { sequence, .. } => Some(format!("steps={}", sequence.len())),
            Node::LayerShift {
                layer_name, mode, ..
            } => Some(format!(
                "target={} mode={}",
                layer_name,
                match mode {
                    LayerMode::Hold => "hold",
                    LayerMode::Toggle => "toggle",
                }
            )),
            _ => None,
        }
    }
}

impl Profile {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| PhantomError::Profile(format!("{}: {}", path.display(), e)))?;
        let profile: Profile = serde_json::from_str(&content)?;
        profile.validate()?;
        Ok(profile)
    }

    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(PhantomError::ProfileValidation {
                field: "name".into(),
                message: "profile name cannot be empty".into(),
            });
        }

        if self.version != 1 {
            return Err(PhantomError::ProfileValidation {
                field: "version".into(),
                message: format!("unsupported version {}, expected 1", self.version),
            });
        }

        if self.global_sensitivity <= 0.0 {
            return Err(PhantomError::ProfileValidation {
                field: "global_sensitivity".into(),
                message: "global_sensitivity must be positive".into(),
            });
        }

        let screen = self
            .screen
            .as_ref()
            .ok_or_else(|| PhantomError::ProfileValidation {
                field: "screen".into(),
                message: "screen override is required for fullscreen profiles".into(),
            })?;
        if screen.width == 0 || screen.height == 0 {
            return Err(PhantomError::ProfileValidation {
                field: "screen".into(),
                message: "screen width and height must be greater than zero".into(),
            });
        }

        if self.nodes.is_empty() {
            return Err(PhantomError::ProfileValidation {
                field: "nodes".into(),
                message: "profile has no nodes".into(),
            });
        }

        let mut ids = HashSet::new();
        let mut slots = HashSet::new();
        let mut keys_by_name: HashMap<String, HashSet<String>> = HashMap::new();
        let mut layer_switch_keys = HashSet::new();

        for node in &self.nodes {
            if !ids.insert(node.id()) {
                return Err(PhantomError::ProfileValidation {
                    field: format!("nodes.{}.id", node.id()),
                    message: format!("duplicate node id '{}'", node.id()),
                });
            }

            if let Some(slot) = node.slot() {
                if !slots.insert(slot) {
                    return Err(PhantomError::ProfileValidation {
                        field: format!("nodes.{}.slot", node.id()),
                        message: format!("duplicate slot {}", slot),
                    });
                }
            }

            validate_node(node)?;

            if let Node::LayerShift { key, .. } = node {
                let normalized = normalize_key_name(key);
                if !layer_switch_keys.insert(normalized.clone()) {
                    return Err(PhantomError::ProfileValidation {
                        field: format!("nodes.{}.key", node.id()),
                        message: format!("key '{}' bound by multiple layer switches", key),
                    });
                }
            } else {
                let layer = node.layer().trim().to_string();
                for key in node.bound_keys() {
                    let normalized = normalize_key_name(key);
                    keys_by_name
                        .entry(normalized)
                        .or_default()
                        .insert(layer.clone());
                }
            }
        }

        for key in &layer_switch_keys {
            if keys_by_name.contains_key(key) {
                return Err(PhantomError::ProfileValidation {
                    field: "nodes.key".into(),
                    message: format!("key '{}' cannot be both a layer switch and an action", key),
                });
            }
        }

        for (key, layers) in keys_by_name {
            if layers.len() > 1 && layers.contains("") {
                return Err(PhantomError::ProfileValidation {
                    field: "nodes.key".into(),
                    message: format!(
                        "key '{}' is bound in the base layer and in a mode layer, which is ambiguous",
                        key
                    ),
                });
            }
        }

        Ok(())
    }

    pub fn audit(&self) -> ProfileAudit {
        let mut touch_entries = Vec::new();
        let mut auxiliary_entries = Vec::new();

        for node in &self.nodes {
            let layer = display_layer_name(node.layer());
            let bindings = node.audit_bindings();
            let detail = node.audit_detail();

            if let Some(slot) = node.slot() {
                touch_entries.push(SlotAuditEntry {
                    slot,
                    node_id: node.id().to_string(),
                    node_type: node.kind(),
                    layer,
                    bindings,
                    detail,
                });
            } else {
                auxiliary_entries.push(AuxiliaryAuditEntry {
                    node_id: node.id().to_string(),
                    node_type: node.kind(),
                    layer,
                    bindings,
                    detail,
                });
            }
        }

        touch_entries.sort_by_key(|entry| entry.slot);
        auxiliary_entries.sort_by(|left, right| left.node_id.cmp(&right.node_id));

        let (screen_width, screen_height) = self
            .screen
            .as_ref()
            .map(|screen| (screen.width, screen.height))
            .unwrap_or((0, 0));

        ProfileAudit {
            profile_name: self.name.clone(),
            screen_width,
            screen_height,
            total_nodes: self.nodes.len(),
            touch_entries,
            auxiliary_entries,
        }
    }
}

fn validate_node(node: &Node) -> Result<()> {
    let id = node.id();
    if id.trim().is_empty() {
        return Err(PhantomError::ProfileValidation {
            field: "nodes.id".into(),
            message: "node id cannot be empty".into(),
        });
    }

    if !matches!(node, Node::LayerShift { .. }) {
        validate_layer_name(node.layer(), &format!("nodes.{id}.layer"))?;
    }

    match node {
        Node::Tap { pos, key, .. }
        | Node::HoldTap { pos, key, .. }
        | Node::ToggleTap { pos, key, .. }
        | Node::RepeatTap { pos, key, .. } => {
            validate_pos(pos, &format!("nodes.{id}.pos"))?;
            validate_key_name(key, &format!("nodes.{id}.key"))?;
        }
        Node::Joystick {
            pos,
            radius,
            mode,
            region,
            keys,
            ..
        } => {
            validate_pos(pos, &format!("nodes.{id}.pos"))?;
            if *radius <= 0.0 || *radius > 1.0 {
                return Err(PhantomError::ProfileValidation {
                    field: format!("nodes.{id}.radius"),
                    message: format!("radius {} must be in (0, 1]", radius),
                });
            }
            match mode {
                JoystickMode::Fixed => {
                    if region.is_some() {
                        return Err(PhantomError::ProfileValidation {
                            field: format!("nodes.{id}.region"),
                            message: "region must be omitted when joystick mode is fixed".into(),
                        });
                    }
                }
                JoystickMode::Floating => {
                    let Some(region) = region.as_ref() else {
                        return Err(PhantomError::ProfileValidation {
                            field: format!("nodes.{id}.region"),
                            message: "region is required when joystick mode is floating".into(),
                        });
                    };
                    validate_region(region, &format!("nodes.{id}.region"))?;
                }
            }
            validate_key_name(&keys.up, &format!("nodes.{id}.keys.up"))?;
            validate_key_name(&keys.down, &format!("nodes.{id}.keys.down"))?;
            validate_key_name(&keys.left, &format!("nodes.{id}.keys.left"))?;
            validate_key_name(&keys.right, &format!("nodes.{id}.keys.right"))?;
        }
        Node::Drag {
            start,
            end,
            key,
            duration_ms,
            ..
        } => {
            validate_pos(start, &format!("nodes.{id}.start"))?;
            validate_pos(end, &format!("nodes.{id}.end"))?;
            validate_key_name(key, &format!("nodes.{id}.key"))?;
            if *duration_ms == 0 {
                return Err(PhantomError::ProfileValidation {
                    field: format!("nodes.{id}.duration_ms"),
                    message: "duration_ms must be greater than zero".into(),
                });
            }
        }
        Node::MouseCamera {
            region,
            sensitivity,
            activation_mode,
            activation_key,
            ..
        } => {
            validate_region(region, &format!("nodes.{id}.region"))?;
            if *sensitivity <= 0.0 {
                return Err(PhantomError::ProfileValidation {
                    field: format!("nodes.{id}.sensitivity"),
                    message: "sensitivity must be positive".into(),
                });
            }
            match activation_mode {
                MouseCameraActivationMode::AlwaysOn => {
                    if activation_key.is_some() {
                        return Err(PhantomError::ProfileValidation {
                            field: format!("nodes.{id}.activation_key"),
                            message:
                                "activation_key must be omitted when activation_mode is always_on"
                                    .into(),
                        });
                    }
                }
                MouseCameraActivationMode::WhileHeld | MouseCameraActivationMode::Toggle => {
                    let Some(key) = activation_key.as_ref() else {
                        return Err(PhantomError::ProfileValidation {
                            field: format!("nodes.{id}.activation_key"),
                            message:
                                "activation_key is required when activation_mode is while_held or toggle"
                                    .into(),
                        });
                    };
                    validate_key_name(key, &format!("nodes.{id}.activation_key"))?;
                }
            }
        }
        Node::Macro { key, sequence, .. } => {
            validate_key_name(key, &format!("nodes.{id}.key"))?;
            if sequence.is_empty() {
                return Err(PhantomError::ProfileValidation {
                    field: format!("nodes.{id}.sequence"),
                    message: "macro sequence cannot be empty".into(),
                });
            }
            for (i, step) in sequence.iter().enumerate() {
                if matches!(step.action, MacroAction::Down) && step.pos.is_none() {
                    return Err(PhantomError::ProfileValidation {
                        field: format!("nodes.{id}.sequence[{i}].pos"),
                        message: "macro 'down' action requires pos".into(),
                    });
                }
                if let Some(pos) = &step.pos {
                    validate_pos(pos, &format!("nodes.{id}.sequence[{i}].pos"))?;
                }
            }
        }
        Node::LayerShift {
            key, layer_name, ..
        } => {
            validate_key_name(key, &format!("nodes.{id}.key"))?;
            validate_layer_name(layer_name, &format!("nodes.{id}.layer_name"))?;
            if layer_name.trim().is_empty() {
                return Err(PhantomError::ProfileValidation {
                    field: format!("nodes.{id}.layer_name"),
                    message: "layer_name cannot be empty".into(),
                });
            }
        }
    }
    Ok(())
}

fn validate_pos(pos: &RelPos, field: &str) -> Result<()> {
    if pos.x < 0.0 || pos.x > 1.0 || pos.y < 0.0 || pos.y > 1.0 {
        return Err(PhantomError::ProfileValidation {
            field: field.into(),
            message: format!("coordinates ({}, {}) out of range [0, 1]", pos.x, pos.y),
        });
    }
    Ok(())
}

fn validate_region(region: &Region, field: &str) -> Result<()> {
    if region.w <= 0.0 || region.h <= 0.0 {
        return Err(PhantomError::ProfileValidation {
            field: field.into(),
            message: "region dimensions must be positive".into(),
        });
    }
    if region.x < 0.0 || region.y < 0.0 || region.x + region.w > 1.0 || region.y + region.h > 1.0 {
        return Err(PhantomError::ProfileValidation {
            field: field.into(),
            message: "region extends outside [0, 1] bounds".into(),
        });
    }
    Ok(())
}

fn validate_layer_name(layer: &str, field: &str) -> Result<()> {
    if layer.chars().any(char::is_whitespace) {
        return Err(PhantomError::ProfileValidation {
            field: field.into(),
            message: "layer names cannot contain whitespace".into(),
        });
    }
    Ok(())
}

fn validate_key_name(key: &str, field: &str) -> Result<()> {
    if key.parse::<Key>().is_err() {
        return Err(PhantomError::ProfileValidation {
            field: field.into(),
            message: format!("unknown key '{}'", key),
        });
    }
    Ok(())
}

fn normalize_key_name(key: &str) -> String {
    key.trim().to_uppercase()
}

fn display_layer_name(layer: &str) -> String {
    if layer.trim().is_empty() {
        "base".into()
    } else {
        layer.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_profile() -> Profile {
        Profile {
            name: "Test".into(),
            version: 1,
            screen: Some(ScreenOverride {
                width: 1920,
                height: 1080,
            }),
            global_sensitivity: 1.0,
            nodes: vec![Node::Tap {
                id: "jump".into(),
                layer: default_layer(),
                slot: 0,
                pos: RelPos { x: 0.5, y: 0.5 },
                key: "Space".into(),
            }],
        }
    }

    #[test]
    fn valid_profile_passes() {
        assert!(valid_profile().validate().is_ok());
    }

    #[test]
    fn audit_sorts_slots_and_marks_base_layer() {
        let mut profile = valid_profile();
        profile.nodes.push(Node::LayerShift {
            id: "combat".into(),
            key: "LeftAlt".into(),
            layer_name: "combat".into(),
            mode: LayerMode::Toggle,
        });
        profile.nodes.push(Node::HoldTap {
            id: "fire".into(),
            layer: "combat".into(),
            slot: 3,
            pos: RelPos { x: 0.8, y: 0.8 },
            key: "MouseLeft".into(),
        });

        let audit = profile.audit();
        assert_eq!(audit.profile_name, "Test");
        assert_eq!(audit.touch_entries.len(), 2);
        assert_eq!(audit.touch_entries[0].slot, 0);
        assert_eq!(audit.touch_entries[0].layer, "base");
        assert_eq!(audit.touch_entries[1].slot, 3);
        assert_eq!(audit.touch_entries[1].layer, "combat");
        assert_eq!(audit.auxiliary_entries.len(), 1);
        assert_eq!(audit.auxiliary_entries[0].node_type, "layer_shift");
    }

    #[test]
    fn rejects_missing_screen() {
        let mut p = valid_profile();
        p.screen = None;
        assert!(p.validate().is_err());
    }

    #[test]
    fn rejects_bad_version() {
        let mut p = valid_profile();
        p.version = 99;
        assert!(p.validate().is_err());
    }

    #[test]
    fn rejects_duplicate_slot() {
        let mut p = valid_profile();
        p.nodes.push(Node::Tap {
            id: "crouch".into(),
            layer: default_layer(),
            slot: 0,
            pos: RelPos { x: 0.6, y: 0.6 },
            key: "C".into(),
        });
        assert!(p.validate().is_err());
    }

    #[test]
    fn accepts_logical_slots_above_nine() {
        let mut p = valid_profile();
        p.nodes = vec![Node::Tap {
            id: "logical".into(),
            layer: default_layer(),
            slot: 10,
            pos: RelPos { x: 0.5, y: 0.5 },
            key: "A".into(),
        }];
        assert!(p.validate().is_ok());
    }

    #[test]
    fn rejects_bad_coords() {
        let mut p = valid_profile();
        p.nodes = vec![Node::Tap {
            id: "bad".into(),
            layer: default_layer(),
            slot: 0,
            pos: RelPos { x: 1.5, y: -0.1 },
            key: "A".into(),
        }];
        assert!(p.validate().is_err());
    }

    #[test]
    fn rejects_duplicate_keys_between_base_and_layer() {
        let mut p = valid_profile();
        p.nodes.push(Node::Tap {
            id: "alt_jump".into(),
            layer: "combat".into(),
            slot: 1,
            pos: RelPos { x: 0.6, y: 0.6 },
            key: "Space".into(),
        });
        assert!(p.validate().is_err());
    }

    #[test]
    fn allows_duplicate_keys_in_distinct_layers() {
        let mut p = valid_profile();
        p.nodes.clear();
        p.nodes.push(Node::Tap {
            id: "lay1".into(),
            layer: "combat".into(),
            slot: 0,
            pos: RelPos { x: 0.2, y: 0.3 },
            key: "Q".into(),
        });
        p.nodes.push(Node::Tap {
            id: "lay2".into(),
            layer: "vehicle".into(),
            slot: 1,
            pos: RelPos { x: 0.3, y: 0.4 },
            key: "Q".into(),
        });
        assert!(p.validate().is_ok());
    }

    #[test]
    fn rejects_empty_nodes() {
        let mut p = valid_profile();
        p.nodes = vec![];
        assert!(p.validate().is_err());
    }

    #[test]
    fn rejects_bad_region() {
        let mut p = valid_profile();
        p.nodes = vec![Node::MouseCamera {
            id: "cam".into(),
            layer: default_layer(),
            slot: 0,
            region: Region {
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 1.0,
            },
            sensitivity: 1.0,
            activation_mode: MouseCameraActivationMode::AlwaysOn,
            activation_key: None,
            invert_y: false,
        }];
        assert!(p.validate().is_err());
    }

    #[test]
    fn rejects_mouse_look_toggle_without_activation_key() {
        let mut p = valid_profile();
        p.nodes = vec![Node::MouseCamera {
            id: "cam".into(),
            layer: default_layer(),
            slot: 0,
            region: Region {
                x: 0.0,
                y: 0.0,
                w: 0.5,
                h: 1.0,
            },
            sensitivity: 1.0,
            activation_mode: MouseCameraActivationMode::Toggle,
            activation_key: None,
            invert_y: false,
        }];
        assert!(p.validate().is_err());
    }

    #[test]
    fn rejects_mouse_look_activation_key_when_always_on() {
        let mut p = valid_profile();
        p.nodes = vec![Node::MouseCamera {
            id: "cam".into(),
            layer: default_layer(),
            slot: 0,
            region: Region {
                x: 0.0,
                y: 0.0,
                w: 0.5,
                h: 1.0,
            },
            sensitivity: 1.0,
            activation_mode: MouseCameraActivationMode::AlwaysOn,
            activation_key: Some("MouseRight".into()),
            invert_y: false,
        }];
        assert!(p.validate().is_err());
    }

    #[test]
    fn rejects_macro_empty_sequence() {
        let mut p = valid_profile();
        p.nodes = vec![Node::Macro {
            id: "combo".into(),
            layer: default_layer(),
            key: "G".into(),
            sequence: vec![],
        }];
        assert!(p.validate().is_err());
    }

    #[test]
    fn rejects_invalid_key_name() {
        let mut p = valid_profile();
        p.nodes = vec![Node::Tap {
            id: "bad_key".into(),
            layer: default_layer(),
            slot: 0,
            pos: RelPos { x: 0.5, y: 0.5 },
            key: "Nope".into(),
        }];
        assert!(p.validate().is_err());
    }

    #[test]
    fn floating_joystick_requires_region() {
        let mut p = valid_profile();
        p.nodes = vec![Node::Joystick {
            id: "move".into(),
            layer: default_layer(),
            slot: 0,
            pos: RelPos { x: 0.2, y: 0.7 },
            radius: 0.08,
            mode: JoystickMode::Floating,
            region: None,
            keys: JoystickKeys {
                up: "W".into(),
                down: "S".into(),
                left: "A".into(),
                right: "D".into(),
            },
        }];
        assert!(p.validate().is_err());
    }

    #[test]
    fn fixed_joystick_rejects_region() {
        let mut p = valid_profile();
        p.nodes = vec![Node::Joystick {
            id: "move".into(),
            layer: default_layer(),
            slot: 0,
            pos: RelPos { x: 0.2, y: 0.7 },
            radius: 0.08,
            mode: JoystickMode::Fixed,
            region: Some(Region {
                x: 0.0,
                y: 0.4,
                w: 0.4,
                h: 0.5,
            }),
            keys: JoystickKeys {
                up: "W".into(),
                down: "S".into(),
                left: "A".into(),
                right: "D".into(),
            },
        }];
        assert!(p.validate().is_err());
    }

    #[test]
    fn drag_requires_positive_duration() {
        let mut p = valid_profile();
        p.nodes = vec![Node::Drag {
            id: "lane_left".into(),
            layer: default_layer(),
            slot: 0,
            start: RelPos { x: 0.5, y: 0.7 },
            end: RelPos { x: 0.2, y: 0.7 },
            key: "A".into(),
            duration_ms: 0,
        }];
        assert!(p.validate().is_err());
    }
}
