use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

use crate::error::{PhantomError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub version: u32,
    #[serde(default)]
    pub screen: Option<ScreenOverride>,
    #[serde(default = "default_sensitivity")]
    pub global_sensitivity: f64,
    pub nodes: Vec<Node>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenOverride {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Node {
    Tap {
        id: String,
        slot: u8,
        pos: RelPos,
        key: String,
    },
    HoldTap {
        id: String,
        slot: u8,
        pos: RelPos,
        key: String,
    },
    Joystick {
        id: String,
        slot: u8,
        pos: RelPos,
        radius: f64,
        keys: JoystickKeys,
    },
    MouseCamera {
        id: String,
        slot: u8,
        region: Region,
        sensitivity: f64,
        #[serde(default)]
        invert_y: bool,
    },
    RepeatTap {
        id: String,
        slot: u8,
        pos: RelPos,
        key: String,
        interval_ms: u64,
    },
    Macro {
        id: String,
        key: String,
        sequence: Vec<MacroStep>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelPos {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoystickKeys {
    pub up: String,
    pub down: String,
    pub left: String,
    pub right: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Region {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroStep {
    pub action: MacroAction,
    #[serde(default)]
    pub pos: Option<RelPos>,
    pub slot: u8,
    #[serde(default)]
    pub delay_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MacroAction {
    Down,
    Up,
}

fn default_sensitivity() -> f64 {
    1.0
}

impl Node {
    pub fn id(&self) -> &str {
        match self {
            Node::Tap { id, .. } => id,
            Node::HoldTap { id, .. } => id,
            Node::Joystick { id, .. } => id,
            Node::MouseCamera { id, .. } => id,
            Node::RepeatTap { id, .. } => id,
            Node::Macro { id, .. } => id,
        }
    }

    pub fn slot(&self) -> Option<u8> {
        match self {
            Node::Tap { slot, .. } => Some(*slot),
            Node::HoldTap { slot, .. } => Some(*slot),
            Node::Joystick { slot, .. } => Some(*slot),
            Node::MouseCamera { slot, .. } => Some(*slot),
            Node::RepeatTap { slot, .. } => Some(*slot),
            Node::Macro { .. } => None, // macro uses slots in its sequence steps
        }
    }

    pub fn bound_keys(&self) -> Vec<&str> {
        match self {
            Node::Tap { key, .. } => vec![key.as_str()],
            Node::HoldTap { key, .. } => vec![key.as_str()],
            Node::Joystick { keys, .. } => vec![
                keys.up.as_str(),
                keys.down.as_str(),
                keys.left.as_str(),
                keys.right.as_str(),
            ],
            Node::MouseCamera { .. } => vec![],
            Node::RepeatTap { key, .. } => vec![key.as_str()],
            Node::Macro { key, .. } => vec![key.as_str()],
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
        // Version check
        if self.version != 1 {
            return Err(PhantomError::ProfileValidation {
                field: "version".into(),
                message: format!("unsupported version {}, expected 1", self.version),
            });
        }

        // Node count
        if self.nodes.is_empty() {
            return Err(PhantomError::ProfileValidation {
                field: "nodes".into(),
                message: "profile has no nodes".into(),
            });
        }

        // Slot uniqueness
        let mut slots = HashSet::new();
        for node in &self.nodes {
            if let Some(slot) = node.slot() {
                if slot > 9 {
                    return Err(PhantomError::ProfileValidation {
                        field: format!("nodes.{}.slot", node.id()),
                        message: format!("slot {} out of range 0-9", slot),
                    });
                }
                if !slots.insert(slot) {
                    return Err(PhantomError::ProfileValidation {
                        field: format!("nodes.{}.slot", node.id()),
                        message: format!("duplicate slot {}", slot),
                    });
                }
            }
        }

        // Validate each node
        for node in &self.nodes {
            validate_node(node)?;
        }

        // Key uniqueness across nodes
        let mut all_keys = HashSet::new();
        for node in &self.nodes {
            for key in node.bound_keys() {
                if !all_keys.insert(key) {
                    return Err(PhantomError::ProfileValidation {
                        field: format!("nodes.{}.key", node.id()),
                        message: format!("key '{}' bound by multiple nodes", key),
                    });
                }
            }
        }

        Ok(())
    }
}

fn validate_node(node: &Node) -> Result<()> {
    let id = node.id();
    if id.is_empty() {
        return Err(PhantomError::ProfileValidation {
            field: "nodes.id".into(),
            message: "node id cannot be empty".into(),
        });
    }

    match node {
        Node::Tap { pos, .. } | Node::HoldTap { pos, .. } | Node::RepeatTap { pos, .. } => {
            validate_pos(pos, &format!("nodes.{id}.pos"))?;
        }
        Node::Joystick { pos, radius, .. } => {
            validate_pos(pos, &format!("nodes.{id}.pos"))?;
            if *radius <= 0.0 || *radius > 1.0 {
                return Err(PhantomError::ProfileValidation {
                    field: format!("nodes.{id}.radius"),
                    message: format!("radius {} must be in (0, 1]", radius),
                });
            }
        }
        Node::MouseCamera {
            region,
            sensitivity,
            ..
        } => {
            validate_region(region, &format!("nodes.{id}.region"))?;
            if *sensitivity <= 0.0 {
                return Err(PhantomError::ProfileValidation {
                    field: format!("nodes.{id}.sensitivity"),
                    message: "sensitivity must be positive".into(),
                });
            }
        }
        Node::Macro { sequence, .. } => {
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
                if step.slot > 9 {
                    return Err(PhantomError::ProfileValidation {
                        field: format!("nodes.{id}.sequence[{i}].slot"),
                        message: format!("slot {} out of range 0-9", step.slot),
                    });
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_profile() -> Profile {
        Profile {
            name: "Test".into(),
            version: 1,
            screen: None,
            global_sensitivity: 1.0,
            nodes: vec![Node::Tap {
                id: "jump".into(),
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
            slot: 0, // duplicate
            pos: RelPos { x: 0.6, y: 0.6 },
            key: "C".into(),
        });
        assert!(p.validate().is_err());
    }

    #[test]
    fn rejects_slot_out_of_range() {
        let mut p = valid_profile();
        p.nodes = vec![Node::Tap {
            id: "bad".into(),
            slot: 10,
            pos: RelPos { x: 0.5, y: 0.5 },
            key: "A".into(),
        }];
        assert!(p.validate().is_err());
    }

    #[test]
    fn rejects_bad_coords() {
        let mut p = valid_profile();
        p.nodes = vec![Node::Tap {
            id: "bad".into(),
            slot: 0,
            pos: RelPos { x: 1.5, y: -0.1 },
            key: "A".into(),
        }];
        assert!(p.validate().is_err());
    }

    #[test]
    fn rejects_duplicate_keys() {
        let mut p = valid_profile();
        p.nodes.push(Node::Tap {
            id: "crouch".into(),
            slot: 1,
            pos: RelPos { x: 0.6, y: 0.6 },
            key: "Space".into(), // same key as jump
        });
        assert!(p.validate().is_err());
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
            slot: 0,
            region: Region {
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 1.0,
            },
            sensitivity: 1.0,
            invert_y: false,
        }];
        assert!(p.validate().is_err());
    }

    #[test]
    fn rejects_macro_empty_sequence() {
        let mut p = valid_profile();
        p.nodes = vec![Node::Macro {
            id: "combo".into(),
            key: "G".into(),
            sequence: vec![],
        }];
        assert!(p.validate().is_err());
    }
}
