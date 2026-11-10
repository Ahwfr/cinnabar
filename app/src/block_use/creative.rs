//! A Creative catalog selection may outlive its press while the server confirms inventory.

use super::*;
use bevy::prelude::Resource;
use inventory::inventory_ledger::{
    InventoryTarget, PLAYER_INVENTORY_SLOT_COUNT, PlayerInventorySlot,
};
use mod_host::CreativePlacementOffer;
use protocol::NetworkItemStack;
use semantic_input::identity::{InputAuthorityGeneration, InputSequence};
use simulation_clock::identity::PositionAuthorityGeneration;
use std::time::Duration;

#[derive(Resource, Default)]
pub(crate) struct CreativePlacement {
    pub offer: Option<CreativePlacementOffer>,
    pending: Option<Pending>,
    claimed: bool,
}

type StackKey = (i32, u32, i32);

struct Pending {
    game_mode: PlayerGameMode,
    offer: CreativePlacementOffer,
    support: ([i32; 3], u8, u32),
    scope: (u64, i32),
    input_generation: InputAuthorityGeneration,
    position_generation: PositionAuthorityGeneration,
    expires: Duration,
    slot: u8,
    original_slot: u8,
    key: StackKey,
    request: Option<i32>,
    selecting: bool,
}

fn key(stack: &NetworkItemStack) -> StackKey {
    (stack.network_id, stack.metadata, stack.block_runtime_id)
}

fn support(observed: &FrozenBlockObservation) -> ([i32; 3], u8, u32) {
    (
        observed.target.position,
        observed.target.face,
        observed.target.runtime_id,
    )
}

/// Starts an assisted item selection, leaving ordinary placement available otherwise.
pub(super) fn try_start(
    player: &mut client_state::PlayerRuntime,
    context: &mut BlockUseContext,
    observed: &FrozenBlockObservation,
) -> bool {
    let Some(policy) = context.creative.as_deref_mut() else {
        return false;
    };
    let Some(stream) = context.client_world.stream.as_ref() else {
        return false;
    };
    let Some(offer) = policy.offer.clone() else {
        return false;
    };
    let Some(game_mode @ (PlayerGameMode::Creative | PlayerGameMode::Survival)) =
        player.facts.player_game_mode()
    else {
        return false;
    };
    if !PaletteWorld::new(
        stream.collision_store(),
        context.collisions.registry(stream.network_id_mode()),
        stream.current_dimension(),
        Default::default(),
    )
    .observe(|world| world.is_air(offer.position).unwrap_or(false))
    .is_ok_and(|observed| observed.value)
    {
        return false;
    }
    let ledger = player.inventory.ledger();
    let mode = stream.network_id_mode();
    let quality = |stack: &NetworkItemStack| {
        let block = held_block_store_id(stream, stack.block_runtime_id)?;
        (context.collisions.block_identifier(mode, block)? == offer.name.as_str()).then(|| {
            context.collisions.block_canonical_state(mode, block) == Some(offer.states.as_str())
        })
    };
    let selected = observed.selection.slot;
    if ledger.backing_stack(selected).and_then(quality).is_some() {
        policy.pending = None;
        policy.claimed = false;
        return false;
    }
    policy.claimed = false;
    if observed.target.face > 5
        || placement_cell(observed.target.position, observed.target.face) != offer.position
    {
        bevy::log::info!(
            ghost = ?offer.position,
            support = ?observed.target.position,
            face = observed.target.face,
            "Fast Place needs the support face adjoining the ghost"
        );
        policy.pending = None;
        return false;
    }
    if policy.pending.is_some() {
        policy.claimed = true;
        return true;
    }

    let hotbar = (0..protocol::HOTBAR_SLOT_COUNT)
        .filter_map(|slot| {
            let stack = ledger.backing_stack(slot)?;
            Some((quality(stack)?, slot, key(stack)))
        })
        .max_by_key(|(exact, _, _)| *exact);
    let empty = (0..protocol::HOTBAR_SLOT_COUNT)
        .find(|slot| matches!(ledger.slot_state(*slot), Some(PlayerInventorySlot::Empty)));
    let carried = (protocol::HOTBAR_SLOT_COUNT..PLAYER_INVENTORY_SLOT_COUNT as u8)
        .filter_map(|slot| {
            let stack = ledger.backing_stack(slot)?;
            Some((quality(stack)?, slot, key(stack)))
        })
        .max_by_key(|(exact, _, _)| *exact);
    let catalog = empty.zip(ledger.creative_catalog().and_then(|catalog| {
        catalog
            .items
            .iter()
            .filter_map(|item| {
                Some((
                    quality(&item.stack)?,
                    item.creative_network_id,
                    key(&item.stack),
                ))
            })
            .max_by_key(|(exact, _, _)| *exact)
    }));
    let (slot, key, request) = if let Some((_, slot, key)) = hotbar {
        (slot, key, None)
    } else if let Some((_, source, item)) = carried {
        let slot = empty.unwrap_or(selected);
        let request = match player
            .inventory_commands()
            .begin_windowless_hotbar_swap(InventoryTarget::Player(source), slot)
        {
            Ok(request) => request,
            Err(error) => {
                bevy::log::info!(block = %offer.name, source, slot, %error, "Fast Place inventory transfer was refused locally");
                return false;
            }
        };
        (slot, item, Some(request))
    } else {
        if game_mode != PlayerGameMode::Creative {
            bevy::log::info!(block = %offer.name, "Fast Place needs this block in the player's inventory");
            return false;
        }
        let Some((slot, (_, id, item))) = catalog else {
            bevy::log::info!(
                block = %offer.name,
                empty_slot = empty.is_some(),
                "Fast Place could not find a usable hotbar or Creative catalog item"
            );
            return false;
        };
        let request = match player
            .inventory_commands()
            .begin_windowless_creative_hotbar_take(id, slot)
        {
            Ok(request) => request,
            Err(error) => {
                bevy::log::info!(block = %offer.name, slot, %error, "Fast Place inventory request was refused locally");
                return false;
            }
        };
        (slot, item, Some(request))
    };
    policy.claimed = true;
    bevy::log::info!(block = %offer.name, slot, ?request, ?game_mode, "Fast Place waiting for inventory and selection");
    policy.pending = Some(Pending {
        game_mode,
        offer,
        support: support(observed),
        scope: (
            stream.authority().actor_session_id(),
            stream.current_dimension(),
        ),
        input_generation: observed.frame.input_authority_generation,
        position_generation: observed.frame.position_authority_generation,
        expires: context.time.elapsed() + Duration::from_secs(2),
        slot,
        original_slot: selected,
        key,
        request,
        selecting: false,
    });
    true
}

/// Clears any queued press if gameplay lost input ownership.
pub(super) fn clear(policy: Option<&mut CreativePlacement>) {
    if let Some(policy) = policy {
        policy.pending = None;
        policy.claimed = false;
    }
}

/// Resumes one admitted press after backing inventory and equipment are authoritative.
#[allow(
    clippy::too_many_arguments,
    reason = "shares the vanilla build context and its owners"
)]
pub(super) fn resume(
    player: &mut client_state::PlayerRuntime,
    context: &mut BlockUseContext,
    runtime: &mut BlockUseRuntime,
    item_use: &mut client_interaction::item_use::ItemUseRuntime,
    swings: &mut SwingTracker,
    movement: &mut MovementTicker,
    pick: Option<FramePick>,
    held: bool,
    mode: PlayerInputMode,
    authority: (InputAuthorityGeneration, InputSequence),
) -> bool {
    let Some(policy) = context.creative.as_deref_mut() else {
        return false;
    };
    policy.claimed &= held;
    let Some(mut pending) = policy.pending.take() else {
        return policy.claimed;
    };
    let Some(stream) = context.client_world.stream.as_ref() else {
        return true;
    };
    if player.facts.player_game_mode() != Some(pending.game_mode)
        || policy.offer.as_ref() != Some(&pending.offer)
        || pending.scope
            != (
                stream.authority().actor_session_id(),
                stream.current_dimension(),
            )
        || context.time.elapsed() >= pending.expires
        || pending.input_generation != authority.0
        || pending.position_generation != movement.position_authority_identity().1
        || context.client_world.respawn.input_held()
        || (!pending.selecting && player.selected_hotbar_slot() != Some(pending.original_slot))
    {
        bevy::log::info!(block = %pending.offer.name, "Fast Place cancelled after its target, input, mode or deadline changed");
        return true;
    }
    let ledger = player.inventory.ledger();
    if pending
        .request
        .is_some_and(|id| ledger.has_pending_request(id))
    {
        policy.pending = Some(pending);
        return true;
    }
    if ledger.backing_stack(pending.slot).map(key) != Some(pending.key) {
        bevy::log::info!(block = %pending.offer.name, slot = pending.slot, "Fast Place inventory did not confirm the requested block");
        return true;
    }
    if !pending.selecting {
        player
            .inventory
            .queue_local_hotbar_selection(pending.slot, Some(pending.game_mode));
        item_use.retire_reselected_use(pending.slot);
        pending.selecting = true;
    }
    if player.inventory.pending_hotbar_selection().is_some() {
        policy.pending = Some(pending);
        return true;
    }
    if player.selected_hotbar_slot() != Some(pending.slot) {
        return true;
    }
    let Some(state) = movement.build_action_state().filter(|_| {
        movement.accepts_block_interactions()
            && !movement.has_held_release()
            && movement.between_ticks_sample().is_some()
    }) else {
        policy.pending = Some(pending);
        return true;
    };
    let Some(observed) = observe_use_target(
        player,
        context,
        &context.origin,
        mode,
        pending.game_mode == PlayerGameMode::Survival,
        authority,
        movement.position_authority_identity().1,
        runtime,
        pick,
        ItemUseTrigger::PlayerInput,
        &state,
    ) else {
        return true;
    };
    if support(&observed) != pending.support {
        bevy::log::info!(block = %pending.offer.name, "Fast Place support changed before placement");
        return true;
    }
    runtime.forget_press_resolution();
    runtime.selection_changed(&observed.selection);
    let clock = RepeatClock::for_state(context.time.elapsed(), &state, Some(pending.game_mode));
    let mut build = build::BuildInput::new(
        observed,
        state,
        ItemUseTrigger::PlayerInput,
        clock.now,
        clock,
        *context.view,
    );
    build.expected = Some(pending.offer);
    build::advance(
        player, context, runtime, item_use, swings, movement, &mut build,
    );
    runtime.clear_press();
    true
}
