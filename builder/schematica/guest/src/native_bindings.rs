//! Cached, paged observations of the player's normal Keyboard & Mouse settings.

use crate::{hotkeys::Action, input};
use std::cell::RefCell;

#[derive(Default)]
struct Cache {
    revision: Option<u64>,
    collecting: Option<u64>,
    pending: Vec<input::ActionBinding>,
    bindings: Vec<input::ActionBinding>,
}

thread_local! {
    static CACHE: RefCell<Cache> = RefCell::new(Cache::default());
}

/// Collects at most one page per frame and publishes only complete revisions.
pub fn sample() {
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let offset = cache.pending.len() as u32;
        let after = if offset == 0 { cache.revision } else { None };
        let page = match input::read_bindings(after, offset, 16) {
            Ok(Some(page)) => page,
            Ok(None) => return,
            Err(_) => {
                *cache = Cache::default();
                return;
            }
        };
        if cache.revision != Some(page.revision) {
            cache.bindings.clear();
        }
        if cache
            .collecting
            .is_some_and(|revision| revision != page.revision)
        {
            *cache = Cache::default();
            return;
        }
        cache.collecting = Some(page.revision);
        cache.pending.extend(page.entries);
        if cache.pending.len() == page.total as usize {
            cache.bindings = std::mem::take(&mut cache.pending);
            cache.revision = Some(page.revision);
            cache.collecting = None;
        }
    });
}

fn name(action: Action) -> &'static str {
    match action {
        Action::Main => "schematic-menu",
        Action::Config => "schematic-config",
        Action::Placement => "schematic-placement",
        Action::Verifier => "schematic-verifier",
        Action::Materials => "schematic-materials",
        Action::Render => "schematic-render",
        Action::Ghosts => "schematic-ghosts",
        Action::NextMode => "schematic-next-mode",
        Action::PreviousMode => "schematic-previous-mode",
        Action::PlacementConfig => "schematic-placement-config",
        Action::NextLayer => "schematic-next-layer",
        Action::PreviousLayer => "schematic-previous-layer",
    }
}

fn held(binding: &input::ActionBinding, keys: &input::Controls) -> bool {
    let held = |key: &str| keys.keys_held.iter().any(|value| value == key);
    (!binding.modifiers.shift || held("ShiftLeft") || held("ShiftRight"))
        && (!binding.modifiers.control || held("ControlLeft") || held("ControlRight"))
        && (!binding.modifiers.alt || held("AltLeft") || held("AltRight"))
        && (!binding.modifiers.super_key || held("SuperLeft") || held("SuperRight"))
}

/// A physical press of the named current gameplay binding.
pub fn pressed(action: &str, keys: &input::Controls) -> bool {
    CACHE.with(|cache| {
        cache.borrow().bindings.iter().any(|binding| {
            binding.action == action
                && binding.context == input::BindingContext::Gameplay
                && held(binding, keys)
                && keys.keys_pressed.contains(&binding.key)
        })
    })
}

/// Resolves guest features from the same physical settings rows the native router observes.
pub fn action(keys: &input::Controls, armed: &mut bool) -> Option<Action> {
    if CACHE.with(|cache| cache.borrow().revision.is_none()) {
        for (key, action) in [("KeyM", Action::Main), ("F6", Action::Config)] {
            if keys.keys_pressed.iter().any(|pressed| pressed == key) {
                *armed = false;
                return Some(action);
            }
        }
    }
    for (action, _, _) in crate::hotkeys::BINDINGS {
        if pressed(name(action), keys) {
            *armed = false;
            return Some(action);
        }
    }
    None
}

pub fn caption(action: Action) -> String {
    CACHE.with(|cache| {
        cache
            .borrow()
            .bindings
            .iter()
            .find(|binding| binding.action == name(action))
            .map_or_else(
                || "Unbound".into(),
                |binding| {
                    binding
                        .key
                        .strip_prefix("Key")
                        .unwrap_or(&binding.key)
                        .to_owned()
                },
            )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_remapped_binding_uses_the_new_key_and_can_be_unbound() {
        CACHE.with(|cache| {
            cache.borrow_mut().bindings = vec![input::ActionBinding {
                action: "schematic-menu".into(),
                context: input::BindingContext::Gameplay,
                key: "F4".into(),
                modifiers: input::BindingModifiers {
                    shift: false,
                    control: false,
                    alt: false,
                    super_key: false,
                },
            }]
        });
        let mut keys = input::Controls {
            seconds: 0.,
            focused: true,
            gameplay: true,
            panel_open: false,
            keys_pressed: vec!["KeyM".into()],
            keys_held: vec!["KeyM".into()],
            keys_admitted: vec![],
            events: vec![],
            pointer: None,
            wheel: 0.,
        };
        assert!(action(&keys, &mut false).is_none());
        keys.keys_pressed = vec!["F4".into()];
        assert!(action(&keys, &mut false) == Some(Action::Main));
        CACHE.with(|cache| cache.borrow_mut().bindings.clear());
        assert!(action(&keys, &mut false).is_none());
    }
}
