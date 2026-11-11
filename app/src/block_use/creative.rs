//! A Creative catalog selection may outlive its press while the server confirms inventory.

use super::*;
use bevy::prelude::{Quat, Resource, Vec3};
use inventory::inventory_ledger::{
    CellGesture, CreativeDestination, InventoryTarget, PLAYER_INVENTORY_SLOT_COUNT,
    PlayerInventorySlot,
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
    catalog: Option<Catalog>,
    selecting: bool,
    aim: Option<TemporaryAim>,
}

#[derive(Clone, Copy)]
struct TemporaryAim {
    player_view: Quat,
    rotation: Quat,
    tick: u64,
    support: super::orientation::Support,
}

impl TemporaryAim {
    fn still_current(self, view: &LocalViewPose) -> bool {
        view.rotation().dot(self.player_view).abs() >= 0.999_999
    }

    fn written(self, movement: &MovementTicker) -> bool {
        let [pitch, yaw] = super::orientation::wire_facing(self.rotation);
        movement
            .latest_written_facing()
            .is_some_and(|(tick, [sent_pitch, sent_yaw])| {
                let yaw_error = (sent_yaw - yaw + 180.0).rem_euclid(360.0) - 180.0;
                tick >= self.tick && (sent_pitch - pitch).abs() <= 0.05 && yaw_error.abs() <= 0.05
            })
    }
}

#[derive(Clone, Copy, Debug)]
enum Catalog {
    Opening(u32),
    CarriedOpening(u8),
    Taking(i32),
    Moving(i32),
    Closing,
}

enum CatalogStep {
    Waiting,
    Ready,
    Failed,
}

fn advance_catalog(player: &mut client_state::PlayerRuntime, pending: &mut Pending) -> CatalogStep {
    let Some(stage) = pending.catalog else {
        return CatalogStep::Ready;
    };
    let ledger = player.inventory_commands();
    if matches!(stage, Catalog::Closing) {
        return if ledger.background_personal_busy() {
            CatalogStep::Waiting
        } else {
            pending.catalog = None;
            CatalogStep::Ready
        };
    }
    if !ledger.background_personal_active() {
        return CatalogStep::Failed;
    }
    let next = match stage {
        Catalog::CarriedOpening(source) if ledger.background_personal_ready() => ledger
            .begin_hotbar_swap(InventoryTarget::Player(source), pending.slot)
            .map(Catalog::Moving),
        Catalog::Opening(id) if ledger.background_personal_ready() => ledger
            .begin_creative_take(id, CreativeDestination::Cursor, None)
            .map(Catalog::Taking),
        Catalog::Taking(request) if !ledger.has_pending_request(request) => {
            if ledger.cursor_stack().map(key) != Some(pending.key)
                || !matches!(
                    ledger.slot_state(pending.slot),
                    Some(PlayerInventorySlot::Empty)
                )
            {
                ledger.request_background_personal_close();
                return CatalogStep::Failed;
            }
            ledger
                .begin_target_gesture(InventoryTarget::Player(pending.slot), CellGesture::Click)
                .map(Catalog::Moving)
        }
        Catalog::Moving(request) if !ledger.has_pending_request(request) => {
            let confirmed = ledger.backing_stack(pending.slot).map(key) == Some(pending.key);
            ledger.request_background_personal_close();
            if !confirmed {
                return CatalogStep::Failed;
            }
            Ok(Catalog::Closing)
        }
        _ => return CatalogStep::Waiting,
    };
    match next {
        Ok(next) => {
            bevy::log::debug!(
                slot = pending.slot,
                ?next,
                "Fast Place: advancing Creative catalog"
            );
            pending.catalog = Some(next);
            CatalogStep::Waiting
        }
        Err(error) => {
            bevy::log::info!(slot = pending.slot, ?stage, %error, "Fast Place: Creative catalog gesture failed");
            ledger.request_background_personal_close();
            CatalogStep::Failed
        }
    }
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
    movement: &mut MovementTicker,
) -> bool {
    let Some(policy) = context.creative.as_deref_mut() else {
        return false;
    };
    let Some(stream) = context.client_world.stream.as_ref() else {
        return false;
    };
    let Some(offer) = policy.offer.clone() else {
        clear(Some(policy), player, movement);
        bevy::log::debug!(support = ?observed.target.position, face = observed.target.face, "Fast Place: native hit has no schematic offer");
        return false;
    };
    bevy::log::debug!(block = %offer.name, states = %offer.states, ghost = ?offer.position,
        support = ?observed.target.position, face = observed.target.face, "Fast Place: examining use press");
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
        bevy::log::debug!(ghost = ?offer.position, "Fast Place: destination is occupied or not loaded");
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
        clear(Some(policy), player, movement);
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
        .max_by_key(|(exact, slot, _)| (*exact, *slot == selected));
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
    let (slot, key, request, catalog) = if let Some((_, slot, key)) = hotbar {
        (slot, key, None, None)
    } else if let Some((_, source, item)) = carried {
        let slot = empty.unwrap_or(selected);
        if !player
            .inventory_commands()
            .request_background_personal_open(stream.local_player_runtime_id())
        {
            bevy::log::info!(block = %offer.name, source, slot,
                "Fast Place could not open inventory for the carried block");
            return false;
        }
        (slot, item, None, Some(Catalog::CarriedOpening(source)))
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
        if !player
            .inventory_commands()
            .request_background_personal_open(stream.local_player_runtime_id())
        {
            bevy::log::info!(block = %offer.name, slot, "Fast Place could not open Creative inventory");
            return false;
        }
        (slot, item, None, Some(Catalog::Opening(id)))
    };
    policy.claimed = true;
    bevy::log::info!(block = %offer.name, slot, ?request, ?catalog, ?game_mode, item_network_id = key.0,
        metadata = key.1, item_block_runtime_id = key.2, "Fast Place waiting for inventory and selection");
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
        catalog,
        selecting: false,
        aim: None,
    });
    true
}

/// Clears any queued press if gameplay lost input ownership.
pub(super) fn clear(
    policy: Option<&mut CreativePlacement>,
    player: &mut client_state::PlayerRuntime,
    movement: &mut MovementTicker,
) {
    if let Some(policy) = policy {
        if policy
            .pending
            .take()
            .is_some_and(|pending| pending.aim.is_some())
        {
            movement.end_placement_facing();
        }
        policy.claimed = false;
        player
            .inventory
            .ledger_mut()
            .request_background_personal_close();
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
    policy.claimed &= held && policy.offer.is_some();
    let Some(mut pending) = policy.pending.take() else {
        return policy.claimed;
    };
    if policy.offer.as_ref() != Some(&pending.offer) {
        bevy::log::debug!(ghost = ?pending.offer.position, current = ?policy.offer, "Fast Place: schematic offer changed before placement");
        policy.claimed = false;
        movement.end_placement_facing();
        player
            .inventory
            .ledger_mut()
            .request_background_personal_close();
        return false;
    }
    let Some(stream) = context.client_world.stream.as_ref() else {
        movement.end_placement_facing();
        player
            .inventory
            .ledger_mut()
            .request_background_personal_close();
        return true;
    };
    if player.facts.player_game_mode() != Some(pending.game_mode)
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
        movement.end_placement_facing();
        player
            .inventory
            .ledger_mut()
            .request_background_personal_close();
        return true;
    }
    if let Some(aim) = pending.aim {
        if !movement.placement_facing_active()
            || !aim.still_current(&context.view)
            || context.input.movement() != [0.0; 2]
        {
            movement.end_placement_facing();
            policy.claimed = false;
            return false;
        }
        if !aim.written(movement) {
            policy.pending = Some(pending);
            return true;
        }
    }
    match advance_catalog(player, &mut pending) {
        CatalogStep::Waiting => {
            policy.pending = Some(pending);
            return true;
        }
        CatalogStep::Failed => {
            bevy::log::info!(block = %pending.offer.name, "Fast Place: Creative inventory did not supply the block");
            return true;
        }
        CatalogStep::Ready => {}
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
        movement.end_placement_facing();
        bevy::log::info!(block = %pending.offer.name, slot = pending.slot, "Fast Place inventory did not confirm the requested block");
        return true;
    }
    if !pending.selecting {
        bevy::log::debug!(slot = pending.slot, request = ?pending.request, "Fast Place: backing stack ready, queuing equipment selection");
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
        movement.end_placement_facing();
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
        movement.end_placement_facing();
        bevy::log::debug!(
            slot = pending.slot,
            "Fast Place: equipment selected but native support could not be observed"
        );
        return true;
    };
    if support(&observed) != pending.support {
        movement.end_placement_facing();
        bevy::log::info!(block = %pending.offer.name, "Fast Place support changed before placement");
        return true;
    }
    if pending.offer.correct_aim
        && pending.aim.is_none()
        && let Some((rotation, support)) =
            super::orientation::correction(player, context, &observed, &state, &pending.offer)
        && movement.begin_placement_facing(
            super::orientation::wire_facing(rotation),
            (rotation * Vec3::NEG_Z).to_array(),
        )
    {
        let aim = TemporaryAim {
            player_view: context.view.rotation(),
            rotation,
            tick: movement.next_tick(),
            support,
        };
        pending.aim = Some(aim);
        bevy::log::debug!(ghost = ?pending.offer.position, ?support, tick = aim.tick,
            facing = ?super::orientation::wire_facing(rotation),
            "Fast Place: sending a valid placement aim");
        if let Some(policy) = context.creative.as_deref_mut() {
            policy.pending = Some(pending);
        }
        return true;
    }
    let mut view = *context.view;
    let observed = if let Some(aim) = pending.aim {
        let Some(adjusted) = super::orientation::recapture(
            player,
            context,
            observed,
            &state,
            aim.rotation,
            aim.support,
        ) else {
            movement.end_placement_facing();
            return true;
        };
        view.set_rotation(aim.rotation);
        adjusted
    } else {
        observed
    };
    runtime.forget_press_resolution();
    runtime.selection_changed(&observed.selection);
    let clock = RepeatClock::for_state(context.time.elapsed(), &state, Some(pending.game_mode));
    let mut build = build::BuildInput::new(
        observed,
        state,
        ItemUseTrigger::PlayerInput,
        clock.now,
        clock,
        view,
    );
    build.expected = Some(pending.offer);
    bevy::log::debug!(
        slot = pending.slot,
        "Fast Place: equipment admitted, resuming native placement"
    );
    build::advance(
        player, context, runtime, item_use, swings, movement, &mut build,
    );
    movement.end_placement_facing();
    runtime.clear_press();
    true
}
