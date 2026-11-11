//! Explicit operator command placement for floating schematic cells.

use crate::{Viewer, gameplay, input, native_bindings};
use schematica_core::{BlockState, StateValue};

/// Sends at most one keep-only command for a physical use press over known missing terrain.
pub fn click(viewer: &mut Viewer, keys: &input::Controls, native_cell: Option<[i32; 3]>) {
    if !viewer.options.fast_place
        || !viewer.options.air_place
        || native_cell.is_some()
        || !native_bindings::pressed("use", keys)
        || !matches!(gameplay::read_block_target(), Ok(None))
    {
        return;
    }
    let Some((block, pos)) = viewer.aimed_block() else {
        return;
    };
    if !viewer.tiles.iter().any(|tile| tile.is_missing(pos)) {
        return;
    }
    let Some(placement) = viewer.placement else {
        return;
    };
    let block = placement.transform_state(block);
    let command = command(&block, pos);
    let message = match gameplay::request_command(&command) {
        Ok(()) => format!("Air placement requested at {pos:?} | Server confirmation pending"),
        Err(error) => format!("Air placement needs setblock permission | {error}"),
    };
    viewer.show_label(&message);
    let _ = crate::hud::local_message(&message);
}

fn command(block: &BlockState, [x, y, z]: [i32; 3]) -> String {
    let properties = block
        .states
        .iter()
        .map(|(name, value)| {
            let value = match value {
                StateValue::Str(value) => serde_json::to_string(value).unwrap(),
                StateValue::Byte(value) => (*value != 0).to_string(),
                value => value.to_string(),
            };
            format!("{}={value}", serde_json::to_string(name).unwrap())
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("setblock {x} {y} {z} {} [{properties}] keep", block.name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_preserves_states_and_cannot_replace_a_real_block() {
        let block = BlockState::new("minecraft:oak_stairs")
            .with("upside_down_bit", StateValue::Byte(0))
            .with("weirdo_direction", StateValue::Int(2));
        assert_eq!(
            command(&block, [1, 2, 3]),
            "setblock 1 2 3 minecraft:oak_stairs [\"upside_down_bit\"=false,\"weirdo_direction\"=2] keep"
        );
    }
}
