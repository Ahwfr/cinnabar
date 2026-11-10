//! Finds a real aim that produces the planned single-block state.

use super::*;
use bevy::{
    math::EulerRot,
    prelude::{Quat, Vec3},
};
use gameplay::movement::UnsentSampleView;
use mod_host::CreativePlacementOffer;

pub(super) type Support = ([i32; 3], u8, u32);

/// The same look representation as the normal movement and placement systems.
pub(super) fn wire_facing(rotation: Quat) -> [f32; 2] {
    let (yaw, pitch, _) = rotation.to_euler(EulerRot::YXZ);
    [
        -pitch.to_degrees(),
        gameplay::movement::wire_yaw(180.0 - yaw.to_degrees()),
    ]
}

/// A sampled direction still has to pass the native pick and placement checks after turning.
pub(super) fn correction(
    player: &client_state::PlayerRuntime,
    context: &BlockUseContext,
    observed: &FrozenBlockObservation,
    state: &UnsentSampleView,
    expected: &CreativePlacementOffer,
) -> Option<(Quat, Support)> {
    if expected.states == "{}"
        || context.input.movement() != [0.0; 2]
        || state.displacement[0].abs() > 0.01
        || state.displacement[2].abs() > 0.01
        || player.facts.mount_unique_id().is_some()
    {
        return None;
    }
    let stream = context.client_world.stream.as_ref()?;
    let mode = stream.network_id_mode();
    let caps = player.facts.game_mode_capabilities()?;
    if !caps.can_build {
        return None;
    }
    let held = held_block_store_id(stream, observed.selection.item.block_runtime_id())?;
    let pick = crate::interaction_authority::pick_context(
        stream,
        &context.collisions,
        player.facts.player_game_mode(),
        Some(observed.selection.item.block_runtime_id()),
    );
    let authority = stream.authority();
    let overlays = authority.decode_ids(stream.current_dimension());
    let world = PaletteWorld::new(
        stream.collision_store(),
        context.collisions.registry(mode),
        stream.current_dimension(),
        pick,
    )
    .with_cell_overlays(&overlays);
    if !world.is_air(expected.position).ok()? {
        return None;
    }
    let matches = |candidate: &FrozenBlockObservation, view| {
        let surroundings =
            use_surroundings(context, &world, candidate, state.position, state.sneaking);
        if LocalUse::resolve(
            &candidate.selection.item,
            candidate.target.position,
            candidate.target.face,
            &surroundings,
            &caps,
        ) == LocalUse::Interact
        {
            return false;
        }
        predicted_placement(
            &context.collisions,
            stream,
            &world,
            held,
            candidate,
            &surroundings,
            view,
        )
        .is_some_and(|placement| {
            placement.position == expected.position
                && placement.additional.is_empty()
                && context.collisions.block_identifier(mode, placement.block)
                    == Some(expected.name.as_str())
                && context
                    .collisions
                    .block_canonical_state(mode, placement.block)
                    == Some(expected.states.as_str())
        })
    };
    if matches(observed, *context.view) {
        return None;
    }
    let eye = Vec3::from_array(observed.ray.origin);
    let forward = Vec3::from_array(observed.ray.direction).try_normalize()?;
    let widen = |v: Vec3| sim::Vec3::new(f64::from(v.x), f64::from(v.y), f64::from(v.z));
    let mut best = None;
    let mut best_dot = -1.0_f32;
    for face in 0..6 {
        let clicked = placement_cell(expected.position, face ^ 1);
        if world.is_air(clicked).unwrap_or(true) {
            continue;
        }
        for point in face_samples(clicked, face, eye) {
            let Some(direction) = (point - eye).try_normalize() else {
                continue;
            };
            let dot = direction.dot(forward);
            if dot <= best_dot {
                continue;
            }
            let Ok(Some(hit)) =
                world.block_interaction_ray_current(widen(eye), widen(direction), observed.reach)
            else {
                continue;
            };
            if hit.block_pos != clicked || hit.face != face || hit.overlay.is_some() {
                continue;
            }
            let actor = gameplay::melee::pick_actor(
                authority.remote_actors(),
                authority.local_mount_unique_id(),
                observed.ray.origin,
                direction.to_array(),
                observed.reach,
            );
            if !matches!(
                gameplay::melee::classify_block_hit(
                    actor,
                    observed.ray.origin,
                    Some(&hit),
                    observed.reach
                ),
                gameplay::melee::Crosshair::Block
            ) {
                continue;
            }
            let mut candidate = observed.clone();
            candidate.target.position = hit.block_pos;
            candidate.target.face = hit.face;
            candidate.target.runtime_id = hit.runtime_id;
            candidate.target.relative_hit = [
                hit.hit_local.x as f32,
                hit.hit_local.y as f32,
                hit.hit_local.z as f32,
            ];
            candidate.target.hit_world = [
                hit.hit_world.x as f32,
                hit.hit_world.y as f32,
                hit.hit_world.z as f32,
            ];
            if !gameplay::interaction_authority::pick_point_in_range(
                candidate.target.hit_world,
                state.position,
                observed.reach,
            ) {
                continue;
            }
            let rotation = client_presentation::camera::look_at_target(eye, point);
            let mut view = *context.view;
            view.set_rotation(rotation);
            if matches(&candidate, view) {
                best_dot = dot;
                best = Some((view.rotation(), (clicked, face, hit.runtime_id)));
            }
        }
    }
    best
}

/// Recasts the published server-facing direction from the current eye before using it.
pub(super) fn recapture(
    player: &client_state::PlayerRuntime,
    context: &BlockUseContext,
    mut observed: FrozenBlockObservation,
    state: &UnsentSampleView,
    rotation: Quat,
    support: Support,
) -> Option<FrozenBlockObservation> {
    let stream = context.client_world.stream.as_ref()?;
    let authority = stream.authority();
    let overlays = authority.decode_ids(stream.current_dimension());
    let pick = crate::interaction_authority::pick_context(
        stream,
        &context.collisions,
        player.facts.player_game_mode(),
        Some(observed.selection.item.block_runtime_id()),
    );
    let world = PaletteWorld::new(
        stream.collision_store(),
        context.collisions.registry(stream.network_id_mode()),
        stream.current_dimension(),
        pick,
    )
    .with_cell_overlays(&overlays);
    let direction = (rotation * Vec3::NEG_Z).to_array();
    let widen = |v: [f32; 3]| sim::Vec3::new(f64::from(v[0]), f64::from(v[1]), f64::from(v[2]));
    let hit = world
        .block_interaction_ray_current(widen(observed.ray.origin), widen(direction), observed.reach)
        .ok()??;
    if (hit.block_pos, hit.face, hit.runtime_id) != support || hit.overlay.is_some() {
        return None;
    }
    let actor = gameplay::melee::pick_actor(
        authority.remote_actors(),
        authority.local_mount_unique_id(),
        observed.ray.origin,
        direction,
        observed.reach,
    );
    if !matches!(
        gameplay::melee::classify_block_hit(actor, observed.ray.origin, Some(&hit), observed.reach,),
        gameplay::melee::Crosshair::Block
    ) {
        return None;
    }
    let hit_world = [
        hit.hit_world.x as f32,
        hit.hit_world.y as f32,
        hit.hit_world.z as f32,
    ];
    if !gameplay::interaction_authority::pick_point_in_range(
        hit_world,
        state.position,
        observed.reach,
    ) {
        return None;
    }
    observed.ray.direction = direction;
    observed.ray.world_identity = hit.identity.clone();
    observed.target = gameplay::mining::FrozenMiningTarget {
        position: hit.block_pos,
        face: hit.face,
        relative_hit: [
            hit.hit_local.x as f32,
            hit.hit_local.y as f32,
            hit.hit_local.z as f32,
        ],
        hit_world,
        runtime_id: hit.runtime_id,
        identity: hit.identity,
    };
    Some(observed)
}

/// Samples both halves and the center of the face, omitting faces the eye is behind.
fn face_samples(clicked: [i32; 3], face: u8, eye: Vec3) -> impl Iterator<Item = Vec3> {
    const SAMPLES: [f32; 5] = [0.5, 0.25, 0.75, 0.0625, 0.9375];
    let axis = [1, 2, 0][usize::from(face / 2)];
    let u = (axis + 1) % 3;
    let v = (axis + 2) % 3;
    let side = f32::from(face & 1);
    let mut origin = clicked.map(|n| n as f32);
    origin[axis] += side;
    let visible = if side == 0.0 {
        eye[axis] < origin[axis]
    } else {
        eye[axis] > origin[axis]
    };
    SAMPLES
        .into_iter()
        .filter(move |_| visible)
        .flat_map(move |a| {
            SAMPLES.into_iter().map(move |b| {
                let mut point = origin;
                point[u] += a;
                point[v] += b;
                Vec3::from_array(point)
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_points_cover_both_halves_of_a_visible_side() {
        let face: Vec<_> = face_samples([4, 7, 9], 5, Vec3::new(7.0, 8.0, 9.5)).collect();
        assert!(face.iter().all(|p| p.x == 5.0));
        assert!(face.iter().any(|p| p.y < 7.5));
        assert!(face.iter().any(|p| p.y > 7.5));
        assert_eq!(
            face_samples([4, 7, 9], 4, Vec3::new(7.0, 8.0, 9.5)).count(),
            0
        );
    }
}
