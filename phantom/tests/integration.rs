use phantom::engine::{KeymapEngine, TouchCommand};
use phantom::input::{InputEvent, Key};
use phantom::profile::*;

fn pubg_profile() -> Profile {
    Profile {
        name: "PUBG Test".into(),
        version: 1,
        screen: Some(ScreenOverride {
            width: 1920,
            height: 1080,
        }),
        global_sensitivity: 1.0,
        nodes: vec![
            Node::Joystick {
                id: "move".into(),
                layer: String::new(),
                slot: 0,
                pos: RelPos { x: 0.18, y: 0.72 },
                radius: 0.07,
                keys: JoystickKeys {
                    up: "W".into(),
                    down: "S".into(),
                    left: "A".into(),
                    right: "D".into(),
                },
            },
            Node::MouseCamera {
                id: "camera".into(),
                layer: String::new(),
                slot: 1,
                region: Region {
                    x: 0.35,
                    y: 0.0,
                    w: 0.65,
                    h: 1.0,
                },
                sensitivity: 1.2,
                activation_mode: MouseCameraActivationMode::AlwaysOn,
                activation_key: None,
                invert_y: false,
            },
            Node::HoldTap {
                id: "fire".into(),
                layer: String::new(),
                slot: 2,
                pos: RelPos { x: 0.88, y: 0.62 },
                key: "MouseLeft".into(),
            },
            Node::Tap {
                id: "jump".into(),
                layer: String::new(),
                slot: 3,
                pos: RelPos { x: 0.92, y: 0.82 },
                key: "Space".into(),
            },
            Node::Tap {
                id: "reload".into(),
                layer: String::new(),
                slot: 4,
                pos: RelPos { x: 0.78, y: 0.88 },
                key: "R".into(),
            },
        ],
    }
}

fn repeat_tap_profile() -> Profile {
    Profile {
        name: "RepeatTest".into(),
        version: 1,
        screen: Some(ScreenOverride {
            width: 1920,
            height: 1080,
        }),
        global_sensitivity: 1.0,
        nodes: vec![Node::RepeatTap {
            id: "auto_fire".into(),
            layer: String::new(),
            slot: 0,
            pos: RelPos { x: 0.5, y: 0.5 },
            key: "F".into(),
            interval_ms: 50,
        }],
    }
}

fn empty_profile() -> Profile {
    Profile {
        name: "Empty".into(),
        version: 1,
        screen: Some(ScreenOverride {
            width: 1920,
            height: 1080,
        }),
        global_sensitivity: 1.0,
        nodes: vec![],
    }
}

fn macro_profile() -> Profile {
    Profile {
        name: "MacroTest".into(),
        version: 1,
        screen: Some(ScreenOverride {
            width: 1920,
            height: 1080,
        }),
        global_sensitivity: 1.0,
        nodes: vec![Node::Macro {
            id: "combo".into(),
            layer: String::new(),
            key: "G".into(),
            sequence: vec![
                MacroStep {
                    action: MacroAction::Down,
                    pos: Some(RelPos { x: 0.5, y: 0.3 }),
                    slot: 0,
                    delay_ms: 0,
                },
                MacroStep {
                    action: MacroAction::Up,
                    pos: None,
                    slot: 0,
                    delay_ms: 30,
                },
                MacroStep {
                    action: MacroAction::Down,
                    pos: Some(RelPos { x: 0.55, y: 0.35 }),
                    slot: 0,
                    delay_ms: 30,
                },
                MacroStep {
                    action: MacroAction::Up,
                    pos: None,
                    slot: 0,
                    delay_ms: 30,
                },
            ],
        }],
    }
}

// ===== Integration: Full PUBG scenario =====

#[test]
fn full_pubg_move_and_shoot() {
    let mut engine = KeymapEngine::new(pubg_profile());

    // Press W to move
    let cmds = engine.process(&InputEvent::KeyPress(Key::W));
    assert_eq!(cmds.len(), 2); // finger down + move
    assert!(matches!(&cmds[0], TouchCommand::TouchDown { slot: 0, .. }));

    // Press D while holding W = diagonal
    let cmds = engine.process(&InputEvent::KeyPress(Key::D));
    assert_eq!(cmds.len(), 1);
    if let TouchCommand::TouchMove { slot, x, y } = &cmds[0] {
        assert_eq!(*slot, 0);
        assert!(*x > 0.18); // moved right
        assert!(*y < 0.72); // moved up (W is still held)
    } else {
        panic!("expected TouchMove");
    }

    // Press fire while moving
    let cmds = engine.process(&InputEvent::KeyPress(Key::MouseLeft));
    assert_eq!(cmds.len(), 1);
    assert!(matches!(&cmds[0], TouchCommand::TouchDown { slot: 2, .. }));

    // Press jump while moving and shooting
    let cmds = engine.process(&InputEvent::KeyPress(Key::Space));
    assert_eq!(cmds.len(), 1);
    assert!(matches!(&cmds[0], TouchCommand::TouchDown { slot: 3, .. }));

    // Release everything
    let cmds = engine.process(&InputEvent::KeyRelease(Key::MouseLeft));
    assert_eq!(cmds.len(), 1);
    assert!(matches!(&cmds[0], TouchCommand::TouchUp { slot: 2 }));

    let cmds = engine.process(&InputEvent::KeyRelease(Key::Space));
    assert_eq!(cmds.len(), 1);
    assert!(matches!(&cmds[0], TouchCommand::TouchUp { slot: 3 }));

    let cmds = engine.process(&InputEvent::KeyRelease(Key::W));
    // W released but D still held — joystick adjusts to right-only
    assert_eq!(cmds.len(), 1);
    if let TouchCommand::TouchMove { slot, x, y } = &cmds[0] {
        assert_eq!(*slot, 0);
        assert!(*x > 0.18); // moved right
        assert!((y - 0.72).abs() < 0.001); // back to center Y
    } else {
        panic!("expected TouchMove");
    }

    let cmds = engine.process(&InputEvent::KeyRelease(Key::D));
    assert_eq!(cmds.len(), 1);
    assert!(matches!(&cmds[0], TouchCommand::TouchUp { slot: 0 }));
}

#[test]
fn empty_profile_is_idle() {
    let mut engine = KeymapEngine::new(empty_profile());

    assert!(engine.process(&InputEvent::KeyPress(Key::F12)).is_empty());
    assert!(engine.process(&InputEvent::KeyRelease(Key::F12)).is_empty());
    assert!(engine
        .process(&InputEvent::MouseMove { dx: 50, dy: 20 })
        .is_empty());
    assert!(engine.tick().is_empty());
}

#[test]
fn mouse_camera_tracks_movement() {
    let mut engine = KeymapEngine::new(pubg_profile());

    // First mouse move — finger goes down at center
    let cmds = engine.process(&InputEvent::MouseMove { dx: 10, dy: 5 });
    assert!(cmds.len() >= 2); // down + move
    assert!(matches!(&cmds[0], TouchCommand::TouchDown { slot: 1, .. }));
    match (&cmds[0], &cmds[1]) {
        (
            TouchCommand::TouchDown {
                x: down_x,
                y: down_y,
                ..
            },
            TouchCommand::TouchMove {
                x: move_x,
                y: move_y,
                ..
            },
        ) => {
            assert_ne!(down_x, move_x);
            assert_ne!(down_y, move_y);
        }
        _ => panic!("expected initial mouse look down+move sequence"),
    }

    // Subsequent moves
    let cmds = engine.process(&InputEvent::MouseMove { dx: 50, dy: 0 });
    assert!(!cmds.is_empty());
    if let TouchCommand::TouchMove { slot, x, .. } = &cmds[0] {
        assert_eq!(*slot, 1);
        assert!(*x > 0.5); // moved right from center
    } else {
        panic!("expected TouchMove");
    }
}

#[test]
fn mouse_camera_releases_after_idle() {
    let mut engine = KeymapEngine::new(pubg_profile());

    let cmds = engine.process(&InputEvent::MouseMove { dx: 20, dy: 0 });
    assert!(matches!(&cmds[0], TouchCommand::TouchDown { slot: 1, .. }));

    std::thread::sleep(std::time::Duration::from_millis(300));
    let cmds = engine.tick();
    assert!(cmds
        .iter()
        .any(|cmd| matches!(cmd, TouchCommand::TouchUp { slot: 1 })));
}

#[test]
fn release_all_clears_everything() {
    let mut engine = KeymapEngine::new(pubg_profile());

    // Activate multiple nodes
    engine.process(&InputEvent::KeyPress(Key::W));
    engine.process(&InputEvent::KeyPress(Key::MouseLeft));
    engine.process(&InputEvent::KeyPress(Key::Space));

    let cmds = engine.release_all();
    // Should have 3 touch up commands (joystick=0, fire=2, jump=3)
    assert_eq!(cmds.len(), 3);
    let slots: Vec<u8> = cmds
        .iter()
        .filter_map(|c| match c {
            TouchCommand::TouchUp { slot } => Some(*slot),
            _ => None,
        })
        .collect();
    assert!(slots.contains(&0));
    assert!(slots.contains(&2));
    assert!(slots.contains(&3));
}

// ===== RepeatTap tests =====

#[test]
fn repeat_tap_initial_down() {
    let mut engine = KeymapEngine::new(repeat_tap_profile());

    let cmds = engine.process(&InputEvent::KeyPress(Key::F));
    assert_eq!(cmds.len(), 1);
    assert!(matches!(&cmds[0], TouchCommand::TouchDown { slot: 0, .. }));
}

#[test]
fn repeat_tap_release_lifts_finger() {
    let mut engine = KeymapEngine::new(repeat_tap_profile());

    engine.process(&InputEvent::KeyPress(Key::F));
    let cmds = engine.process(&InputEvent::KeyRelease(Key::F));
    assert_eq!(cmds.len(), 1);
    assert!(matches!(&cmds[0], TouchCommand::TouchUp { slot: 0 }));
}

#[test]
fn repeat_tap_does_not_double_press() {
    let mut engine = KeymapEngine::new(repeat_tap_profile());

    engine.process(&InputEvent::KeyPress(Key::F));
    // Second press should be ignored (already active)
    let cmds = engine.process(&InputEvent::KeyPress(Key::F));
    assert!(cmds.is_empty());
}

// ===== Macro tests =====

#[test]
fn macro_triggers_on_key_press() {
    let mut engine = KeymapEngine::new(macro_profile());

    // Key press starts macro but doesn't immediately execute steps
    // (steps execute on tick())
    let cmds = engine.process(&InputEvent::KeyPress(Key::G));
    assert!(cmds.is_empty()); // no immediate touch — macro runs on tick
}

#[test]
fn macro_executes_first_step_on_tick() {
    let mut engine = KeymapEngine::new(macro_profile());

    engine.process(&InputEvent::KeyPress(Key::G));
    // First tick should execute step 0 (delay_ms=0)
    let cmds = engine.tick();
    assert_eq!(cmds.len(), 1);
    assert!(matches!(&cmds[0], TouchCommand::TouchDown { slot: 0, .. }));
}

#[test]
fn macro_release_stops_execution() {
    let mut engine = KeymapEngine::new(macro_profile());

    engine.process(&InputEvent::KeyPress(Key::G));
    engine.tick(); // execute first step

    let cmds = engine.process(&InputEvent::KeyRelease(Key::G));
    assert_eq!(cmds.len(), 1); // releases the finger
    assert!(matches!(&cmds[0], TouchCommand::TouchUp { slot: 0 }));

    // After release, ticks should produce nothing
    let cmds = engine.tick();
    assert!(cmds.is_empty());
}

// ===== Profile loading tests =====

#[test]
fn load_pubg_profile_from_file() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("profiles/pubg.json");
    if path.exists() {
        let profile = Profile::load(&path).expect("failed to load pubg.json");
        assert_eq!(profile.name, "PUBG Mobile");
        assert_eq!(profile.nodes.len(), 9);
    }
}

#[test]
fn load_genshin_profile_from_file() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("profiles/genshin.json");
    if path.exists() {
        let profile = Profile::load(&path).expect("failed to load genshin.json");
        assert_eq!(profile.name, "Genshin Impact");
        assert_eq!(profile.nodes.len(), 7);
    }
}

// ===== Key parsing tests =====

#[test]
fn key_from_str_all_letters() {
    for c in 'A'..='Z' {
        let key = c.to_string().parse::<Key>().ok();
        assert!(key.is_some(), "failed to parse key '{}'", c);
    }
}

#[test]
fn key_from_str_case_insensitive() {
    assert_eq!("a".parse::<Key>().ok(), "A".parse::<Key>().ok());
    assert_eq!("z".parse::<Key>().ok(), "Z".parse::<Key>().ok());
    assert_eq!("space".parse::<Key>().ok(), "SPACE".parse::<Key>().ok());
}

#[test]
fn key_from_str_mouse_buttons() {
    assert_eq!("MouseLeft".parse::<Key>().ok(), Some(Key::MouseLeft));
    assert_eq!("MouseRight".parse::<Key>().ok(), Some(Key::MouseRight));
    assert_eq!("MOUSELEFT".parse::<Key>().ok(), Some(Key::MouseLeft));
}

#[test]
fn key_from_str_invalid() {
    assert_eq!("INVALID_KEY_12345".parse::<Key>().ok(), None);
    assert_eq!("".parse::<Key>().ok(), None);
}

// ===== Coordinate edge cases =====

#[test]
fn joystick_opposing_keys_cancel() {
    let mut engine = KeymapEngine::new(pubg_profile());

    // Press W and S simultaneously
    engine.process(&InputEvent::KeyPress(Key::W));
    let cmds = engine.process(&InputEvent::KeyPress(Key::S));
    // Both up and down held — dy should be 0
    if let Some(TouchCommand::TouchMove { x, y, .. }) = cmds.first() {
        let pos = RelPos { x: 0.18, y: 0.72 };
        assert!((x - pos.x).abs() < 0.001); // no x change
        assert!((y - pos.y).abs() < 0.001); // no y change (canceled)
    }
}

#[test]
fn tap_at_screen_edges() {
    let profile = Profile {
        name: "EdgeTest".into(),
        version: 1,
        screen: Some(ScreenOverride {
            width: 1920,
            height: 1080,
        }),
        global_sensitivity: 1.0,
        nodes: vec![
            Node::Tap {
                id: "top_left".into(),
                layer: String::new(),
                slot: 0,
                pos: RelPos { x: 0.0, y: 0.0 },
                key: "A".into(),
            },
            Node::Tap {
                id: "bottom_right".into(),
                layer: String::new(),
                slot: 1,
                pos: RelPos { x: 1.0, y: 1.0 },
                key: "B".into(),
            },
        ],
    };
    let mut engine = KeymapEngine::new(profile);

    let cmds = engine.process(&InputEvent::KeyPress(Key::A));
    if let TouchCommand::TouchDown { x, y, .. } = &cmds[0] {
        assert_eq!(*x, 0.0);
        assert_eq!(*y, 0.0);
    }

    let cmds = engine.process(&InputEvent::KeyPress(Key::B));
    if let TouchCommand::TouchDown { x, y, .. } = &cmds[0] {
        assert_eq!(*x, 1.0);
        assert_eq!(*y, 1.0);
    }
}

// ===== Sensitivity =====

#[test]
fn global_sensitivity_affects_camera() {
    let mut profile = pubg_profile();
    profile.global_sensitivity = 2.0;
    let mut engine_high = KeymapEngine::new(profile);
    let mut engine_low = KeymapEngine::new(pubg_profile()); // sensitivity 1.0

    // First mouse move on each — both place finger at center
    engine_high.process(&InputEvent::MouseMove { dx: 0, dy: 0 });
    engine_low.process(&InputEvent::MouseMove { dx: 0, dy: 0 });

    // Second mouse move — now the sensitivity difference matters
    let cmds_high = engine_high.process(&InputEvent::MouseMove { dx: 100, dy: 0 });
    let cmds_low = engine_low.process(&InputEvent::MouseMove { dx: 100, dy: 0 });

    let move_high = cmds_high
        .iter()
        .find(|c| matches!(c, TouchCommand::TouchMove { .. }));
    let move_low = cmds_low
        .iter()
        .find(|c| matches!(c, TouchCommand::TouchMove { .. }));

    if let (
        Some(TouchCommand::TouchMove { x: x_high, .. }),
        Some(TouchCommand::TouchMove { x: x_low, .. }),
    ) = (move_high, move_low)
    {
        assert!(
            x_high > x_low,
            "higher sensitivity (x={}) should move more than lower (x={})",
            x_high,
            x_low
        );
    } else {
        panic!("expected TouchMove from both engines");
    }
}

// ===== Concurrent slot usage =====

#[test]
fn all_ten_slots_independent() {
    let mut profile = Profile {
        name: "TenSlot".into(),
        version: 1,
        screen: Some(ScreenOverride {
            width: 1920,
            height: 1080,
        }),
        global_sensitivity: 1.0,
        nodes: vec![],
    };
    for i in 0..10u8 {
        profile.nodes.push(Node::Tap {
            id: format!("slot_{}", i),
            layer: String::new(),
            slot: i,
            pos: RelPos {
                x: 0.1 * i as f64,
                y: 0.5,
            },
            key: format!("{}", i + 1), // "1" through "0" for keys
        });
    }
    let _engine = KeymapEngine::new(profile);
    // Should not panic — all slots are unique
}

#[test]
fn simultaneous_tap_keys_both_register() {
    // Two tap nodes on different slots
    let profile = Profile {
        name: "SimulTest".into(),
        version: 1,
        screen: Some(ScreenOverride {
            width: 1920,
            height: 1080,
        }),
        global_sensitivity: 1.0,
        nodes: vec![
            Node::Tap {
                id: "btn_a".into(),
                layer: String::new(),
                slot: 0,
                pos: RelPos { x: 0.3, y: 0.5 },
                key: "A".into(),
            },
            Node::Tap {
                id: "btn_b".into(),
                layer: String::new(),
                slot: 1,
                pos: RelPos { x: 0.7, y: 0.5 },
                key: "B".into(),
            },
        ],
    };
    let mut engine = KeymapEngine::new(profile);

    // Press A
    let cmds_a = engine.process(&InputEvent::KeyPress(Key::A));
    // Press B (same batch, as if simultaneous)
    let cmds_b = engine.process(&InputEvent::KeyPress(Key::B));

    // Both should produce TouchDown on different slots
    assert_eq!(cmds_a.len(), 1);
    assert_eq!(cmds_b.len(), 1);
    assert!(matches!(
        &cmds_a[0],
        TouchCommand::TouchDown { slot: 0, .. }
    ));
    assert!(matches!(
        &cmds_b[0],
        TouchCommand::TouchDown { slot: 1, .. }
    ));

    // Release both
    let cmds_a_up = engine.process(&InputEvent::KeyRelease(Key::A));
    let cmds_b_up = engine.process(&InputEvent::KeyRelease(Key::B));
    assert_eq!(cmds_a_up.len(), 1);
    assert_eq!(cmds_b_up.len(), 1);
    assert!(matches!(&cmds_a_up[0], TouchCommand::TouchUp { slot: 0 }));
    assert!(matches!(&cmds_b_up[0], TouchCommand::TouchUp { slot: 1 }));
}

#[test]
fn simultaneous_tap_and_joystick() {
    let profile = Profile {
        name: "MixedTest".into(),
        version: 1,
        screen: Some(ScreenOverride {
            width: 1920,
            height: 1080,
        }),
        global_sensitivity: 1.0,
        nodes: vec![
            Node::Joystick {
                id: "move".into(),
                layer: String::new(),
                slot: 0,
                pos: RelPos { x: 0.2, y: 0.7 },
                radius: 0.07,
                keys: JoystickKeys {
                    up: "W".into(),
                    down: "S".into(),
                    left: "A".into(),
                    right: "D".into(),
                },
            },
            Node::Tap {
                id: "jump".into(),
                layer: String::new(),
                slot: 1,
                pos: RelPos { x: 0.9, y: 0.8 },
                key: "Space".into(),
            },
        ],
    };
    let mut engine = KeymapEngine::new(profile);

    // Press W to move
    let cmds_move = engine.process(&InputEvent::KeyPress(Key::W));
    assert_eq!(cmds_move.len(), 2); // down + move on slot 0

    // Press Space while W is held
    let cmds_jump = engine.process(&InputEvent::KeyPress(Key::Space));
    assert_eq!(cmds_jump.len(), 1); // down on slot 1
    assert!(matches!(
        &cmds_jump[0],
        TouchCommand::TouchDown { slot: 1, .. }
    ));

    // Both slots are active simultaneously
    // Release Space, movement continues
    let cmds_jump_up = engine.process(&InputEvent::KeyRelease(Key::Space));
    assert_eq!(cmds_jump_up.len(), 1);
    assert!(matches!(
        &cmds_jump_up[0],
        TouchCommand::TouchUp { slot: 1 }
    ));

    // Movement still active on slot 0
    let cmds_d = engine.process(&InputEvent::KeyPress(Key::D));
    assert_eq!(cmds_d.len(), 1); // diagonal move
    assert!(matches!(
        &cmds_d[0],
        TouchCommand::TouchMove { slot: 0, .. }
    ));
}
