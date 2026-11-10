use crate::input;
use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Config,
    Placement,
    Verifier,
    Materials,
    Render,
    Ghosts,
    NextMode,
    PreviousMode,
    PlacementConfig,
    NextLayer,
    PreviousLayer,
    Main,
}

use Action::*;

/// Defaults and order of the editable fields. Chords take precedence over single keys.
pub const BINDINGS: [(Action, &str, &str); 12] = [
    (Config, "Configuration", "M+C"),
    (Placement, "Placement controls", "M+P"),
    (Verifier, "Verifier", "M+V"),
    (Materials, "Materials", "M+L"),
    (Render, "All rendering", "M+R"),
    (Ghosts, "Ghost blocks", "M+G"),
    (NextMode, "Next layer mode", "M+PGUP"),
    (PreviousMode, "Previous layer mode", "M+PGDN"),
    (PlacementConfig, "Placement configuration", "NUM-"),
    (NextLayer, "Next layer", "PGUP"),
    (PreviousLayer, "Previous layer", "PGDN"),
    (Main, "Main menu", "M"),
];

pub fn binding(overrides: &BTreeMap<usize, String>, index: usize) -> &str {
    overrides
        .get(&index)
        .map_or(BINDINGS[index].2, String::as_str)
}

pub fn for_action(overrides: &BTreeMap<usize, String>, action: Action) -> &str {
    let index = BINDINGS.iter().position(|entry| entry.0 == action).unwrap();
    binding(overrides, index)
}

pub fn edit(overrides: &mut BTreeMap<usize, String>, index: usize, value: &str) {
    let value = value.trim().to_ascii_uppercase();
    if index >= BINDINGS.len() || (!value.is_empty() && chord(&value).is_none()) {
        return;
    }
    if let Some((lead, key)) = chord(&value) {
        let duplicate = (0..BINDINGS.len())
            .filter(|&other| other != index)
            .any(|other| {
                chord(binding(overrides, other)).is_some_and(|(other_lead, other_key)| {
                    (lead, key) == (other_lead, other_key)
                        || (lead == Some(other_key) && other_lead == Some(key))
                })
            });
        if duplicate {
            return;
        }
    }
    if value == BINDINGS[index].2 {
        overrides.remove(&index);
    } else {
        overrides.insert(index, value);
    }
}

fn physical(key: &str) -> Option<&str> {
    const LETTERS: [&str; 26] = [
        "KeyA", "KeyB", "KeyC", "KeyD", "KeyE", "KeyF", "KeyG", "KeyH", "KeyI", "KeyJ", "KeyK",
        "KeyL", "KeyM", "KeyN", "KeyO", "KeyP", "KeyQ", "KeyR", "KeyS", "KeyT", "KeyU", "KeyV",
        "KeyW", "KeyX", "KeyY", "KeyZ",
    ];
    Some(match key {
        "PGUP" => "PageUp",
        "PGDN" => "PageDown",
        "NUM-" => "NumpadSubtract",
        "NUM*" => "NumpadMultiply",
        "UP" => "ArrowUp",
        "DOWN" => "ArrowDown",
        "LEFT" => "ArrowLeft",
        "RIGHT" => "ArrowRight",
        key if key.len() == 1 && key.as_bytes()[0].is_ascii_uppercase() => {
            LETTERS[(key.as_bytes()[0] - b'A') as usize]
        }
        key if key
            .strip_prefix('F')
            .and_then(|n| n.parse::<u8>().ok())
            .is_some_and(|n| (1..=12).contains(&n)) =>
        {
            key
        }
        _ => return None,
    })
}

fn chord(value: &str) -> Option<(Option<&str>, &str)> {
    match value.split_once('+') {
        Some((lead, key)) if lead != key => Some((Some(physical(lead)?), physical(key)?)),
        Some(_) => None,
        None => Some((None, physical(value)?)),
    }
}

/// Delay a standalone main-menu key until release so its chords can act first.
pub fn pressed(
    overrides: &BTreeMap<usize, String>,
    keys: &input::Controls,
    menu_armed: &mut bool,
) -> Option<Action> {
    let pressed = |name: &str| keys.keys_pressed.iter().any(|key| key == name);
    let held = |name: &str| keys.keys_held.iter().any(|key| key == name);
    let mut single = None;
    let mut main = None;
    for (index, (action, _, _)) in BINDINGS.into_iter().enumerate() {
        let Some((lead, key)) = chord(binding(overrides, index)) else {
            continue;
        };
        if let Some(lead) = lead {
            if (held(lead) && pressed(key)) || (held(key) && pressed(lead)) {
                *menu_armed = false;
                return Some(action);
            }
        } else if action == Main {
            main = Some(key);
        } else if pressed(key) {
            single = Some(action);
        }
    }
    if let Some(key) = main {
        if pressed(key) {
            *menu_armed = true;
        }
        if !held(key) && std::mem::take(menu_armed) {
            return Some(Main);
        }
    }
    single
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(pressed: &[&str], held: &[&str]) -> input::Controls {
        input::Controls {
            seconds: 0.,
            focused: true,
            gameplay: true,
            panel_open: false,
            keys_pressed: pressed.iter().map(|key| (*key).into()).collect(),
            keys_held: held.iter().map(|key| (*key).into()).collect(),
            keys_admitted: Vec::new(),
            events: Vec::new(),
            pointer: None,
            wheel: 0.,
        }
    }

    #[test]
    fn menu_release_and_chords_resolve_separately() {
        let mut overrides = BTreeMap::new();
        let mut armed = false;
        assert!(pressed(&overrides, &frame(&["KeyM"], &["KeyM"]), &mut armed).is_none());
        assert!(pressed(&overrides, &frame(&[], &[]), &mut armed) == Some(Main));
        assert!(pressed(&overrides, &frame(&["KeyM"], &["KeyM"]), &mut armed).is_none());
        assert!(
            pressed(&overrides, &frame(&["KeyC"], &["KeyM", "KeyC"]), &mut armed) == Some(Config)
        );
        assert!(pressed(&overrides, &frame(&[], &[]), &mut armed).is_none());
        edit(&mut overrides, 1, "c+m");
        assert_eq!(binding(&overrides, 1), "M+P");
        edit(&mut overrides, 0, "f6");
        assert!(pressed(&overrides, &frame(&["F6"], &["F6"]), &mut armed) == Some(Config));
    }
}
