use schematica_core::*;
use std::collections::HashMap;

struct TestWorld(HashMap<[i32; 3], BlockState>, Option<([i32; 3], [i32; 3])>);
impl WorldView for TestWorld {
    fn block_at(&self, p: [i32; 3]) -> Option<&BlockState> {
        // `.1` = loaded AABB; outside it is "unknown", inside it defaults to air
        static AIR: std::sync::OnceLock<BlockState> = std::sync::OnceLock::new();
        if let Some((lo, hi)) = self.1 {
            if (0..3).any(|i| p[i] < lo[i] || p[i] >= hi[i]) {
                return None;
            }
        }
        Some(
            self.0
                .get(&p)
                .unwrap_or_else(|| AIR.get_or_init(BlockState::air)),
        )
    }
}

fn stone() -> BlockState {
    BlockState::new("minecraft:stone")
}

fn asymmetric() -> Schematic {
    // 3 x 2 x 4, every cell distinguishable by position-encoded palette entry
    let mut s = Schematic::new("t", [3, 2, 4]);
    for y in 0..2 {
        for z in 0..4 {
            for x in 0..3 {
                s.set(
                    [x, y, z],
                    BlockState::new("minecraft:stone")
                        .with("id", StateValue::Int(x * 100 + y * 10 + z)),
                );
            }
        }
    }
    s
}

#[test]
fn local_world_roundtrip_all_orientations() {
    let s = asymmetric();
    for turns in 0..4u8 {
        for mirror in [Mirror::None, Mirror::X, Mirror::Z] {
            let p = Placement {
                origin: [10, 64, -5],
                quarter_turns: turns,
                mirror,
            };
            let (min, max) = p.world_bounds(s.size());
            let mut seen = std::collections::HashSet::new();
            for y in 0..2 {
                for z in 0..4 {
                    for x in 0..3 {
                        let w = p.local_to_world(s.size(), [x, y, z]);
                        assert!(
                            (0..3).all(|i| w[i] >= min[i] && w[i] < max[i]),
                            "{turns} {mirror:?} {w:?}"
                        );
                        assert!(seen.insert(w), "collision at {w:?} ({turns} {mirror:?})");
                        assert_eq!(p.world_to_local(s.size(), w), Some([x, y, z]));
                    }
                }
            }
            assert_eq!(seen.len(), 24);
            assert_eq!(p.world_to_local(s.size(), [max[0], min[1], min[2]]), None);
        }
    }
}

#[test]
fn clockwise_turn_moves_north_west_corner_to_north_east() {
    let mut p = Placement::new([0, 0, 0]);
    p.rotate_cw();
    // local NW-bottom corner (0,0,0) in a 3x?x4 box -> NE corner of the new 4x?x3 box
    assert_eq!(p.local_to_world([3, 1, 4], [0, 0, 0]), [3, 0, 0]);
    assert_eq!(p.footprint([3, 1, 4]), [4, 1, 3]);
}

#[test]
fn four_cw_turns_is_identity() {
    let mut p = Placement::new([1, 2, 3]);
    for _ in 0..4 {
        p.rotate_cw();
    }
    assert_eq!(p, Placement::new([1, 2, 3]));
}

fn stairs(dir: i32) -> BlockState {
    BlockState::new("minecraft:oak_stairs")
        .with("weirdo_direction", StateValue::Int(dir))
        .with("upside_down_bit", StateValue::Bool(false))
}

#[test]
fn stairs_rotate_with_the_build() {
    let mut p = Placement::new([0; 3]);
    p.rotate_cw(); // east -> south
    assert_eq!(
        p.transform_state(&stairs(0)).states["weirdo_direction"],
        StateValue::Int(2)
    );
    p.rotate_cw(); // east -> west
    assert_eq!(
        p.transform_state(&stairs(0)).states["weirdo_direction"],
        StateValue::Int(1)
    );
    // unrelated state untouched
    assert_eq!(
        p.transform_state(&stairs(0)).states["upside_down_bit"],
        StateValue::Bool(false)
    );
}

#[test]
fn mirror_flips_only_its_axis() {
    let mut p = Placement::new([0; 3]);
    p.set_mirror(Mirror::X);
    assert_eq!(
        p.transform_state(&stairs(0)).states["weirdo_direction"],
        StateValue::Int(1)
    ); // E->W
    assert_eq!(
        p.transform_state(&stairs(2)).states["weirdo_direction"],
        StateValue::Int(2)
    ); // S stays
    p.set_mirror(Mirror::Z);
    assert_eq!(
        p.transform_state(&stairs(2)).states["weirdo_direction"],
        StateValue::Int(3)
    ); // S->N
}

#[test]
fn trapdoor_uses_its_own_direction_order() {
    let mut p = Placement::new([0; 3]);
    p.rotate_cw();
    let t = BlockState::new("minecraft:oak_trapdoor").with("direction", StateValue::Int(0)); // east
    assert_eq!(
        p.transform_state(&t).states["direction"],
        StateValue::Int(2)
    ); // south
    let r = BlockState::new("minecraft:unpowered_repeater").with("direction", StateValue::Int(0)); // south
    assert_eq!(
        p.transform_state(&r).states["direction"],
        StateValue::Int(1)
    ); // west
}

#[test]
fn axis_facing_cardinal_and_sign_states() {
    let mut p = Placement::new([0; 3]);
    p.rotate_cw();
    let log = BlockState::new("minecraft:oak_log").with("pillar_axis", StateValue::Str("x".into()));
    assert_eq!(
        p.transform_state(&log).states["pillar_axis"],
        StateValue::Str("z".into())
    );
    let up = BlockState::new("minecraft:oak_log").with("pillar_axis", StateValue::Str("y".into()));
    assert_eq!(
        p.transform_state(&up).states["pillar_axis"],
        StateValue::Str("y".into())
    );
    let piston = BlockState::new("minecraft:piston").with("facing_direction", StateValue::Int(2)); // north
    assert_eq!(
        p.transform_state(&piston).states["facing_direction"],
        StateValue::Int(5)
    ); // east
    let piston_up =
        BlockState::new("minecraft:piston").with("facing_direction", StateValue::Int(1));
    assert_eq!(
        p.transform_state(&piston_up).states["facing_direction"],
        StateValue::Int(1)
    );
    let chest = BlockState::new("minecraft:chest").with(
        "minecraft:cardinal_direction",
        StateValue::Str("north".into()),
    );
    assert_eq!(
        p.transform_state(&chest).states["minecraft:cardinal_direction"],
        StateValue::Str("east".into())
    );
    let torch = BlockState::new("minecraft:torch")
        .with("torch_facing_direction", StateValue::Str("top".into()));
    assert_eq!(
        p.transform_state(&torch).states["torch_facing_direction"],
        StateValue::Str("top".into())
    );
    let sign = BlockState::new("minecraft:standing_sign")
        .with("ground_sign_direction", StateValue::Int(0)); // south
    assert_eq!(
        p.transform_state(&sign).states["ground_sign_direction"],
        StateValue::Int(4)
    ); // west
}

#[test]
fn classify_matches_litematica_semantics() {
    let air = BlockState::air();
    let dirt = BlockState::new("minecraft:dirt");
    let m = MatchMode::Exact;
    assert_eq!(classify(&stone(), Some(&stone()), m), CellStatus::Correct);
    assert_eq!(classify(&stone(), Some(&air), m), CellStatus::Missing);
    assert_eq!(classify(&stone(), Some(&dirt), m), CellStatus::Wrong);
    assert_eq!(classify(&air, Some(&dirt), m), CellStatus::Extra);
    assert_eq!(classify(&air, Some(&air), m), CellStatus::Correct);
    assert_eq!(classify(&stone(), None, m), CellStatus::Unloaded);
    let s1 = stairs(0);
    let s2 = stairs(1);
    assert_eq!(
        classify(&s1, Some(&s2), MatchMode::Exact),
        CellStatus::Wrong
    );
    assert_eq!(
        classify(&s1, Some(&s2), MatchMode::IgnoreStates),
        CellStatus::Correct
    );
}

#[test]
fn scan_reports_missing_wrong_and_unloaded() {
    let mut s = Schematic::new("row", [4, 1, 1]);
    for x in 0..4 {
        s.set([x, 0, 0], stone());
    }
    let placed = Placed::new(&s, Placement::new([100, 70, 100]));
    let mut w = HashMap::new();
    w.insert([100, 70, 100], stone()); // correct
    w.insert([101, 70, 100], BlockState::new("minecraft:dirt")); // wrong
                                                                 // [102] air -> missing ; [103] outside loaded AABB -> unloaded
    let world = TestWorld(w, Some(([100, 70, 100], [103, 71, 101])));
    let cells = scan_region(
        &placed,
        &world,
        [0, 0, 0].map(|_| -1000),
        [1000; 3],
        ScanOptions::default(),
    );
    let mut t = Tally::default();
    for c in &cells {
        t.add(c.status);
    }
    assert_eq!(
        t,
        Tally {
            correct: 0,
            missing: 1,
            wrong: 1,
            extra: 0,
            unloaded: 1
        }
    );
    let all = scan_region(
        &placed,
        &world,
        [-1000; 3],
        [1000; 3],
        ScanOptions {
            include_correct: true,
            ..Default::default()
        },
    );
    assert_eq!(all.len(), 4);
}

#[test]
fn scan_works_on_rotated_placement() {
    let mut s = Schematic::new("l", [3, 1, 2]);
    s.set([2, 0, 0], BlockState::new("minecraft:gold_block")); // far-east, north row
    let mut p = Placement::new([0, 0, 0]);
    p.rotate_cw();
    let placed = Placed::new(&s, p);
    let target = p.local_to_world(s.size(), [2, 0, 0]);
    assert_eq!(
        placed.state_at(target).unwrap().name,
        "minecraft:gold_block"
    );
    let world = TestWorld(HashMap::new(), None);
    let cells = scan_region(&placed, &world, [-10; 3], [10; 3], ScanOptions::default());
    assert_eq!(cells.len(), 1);
    assert_eq!(cells[0].pos, target);
    assert_eq!(cells[0].status, CellStatus::Missing);
}

#[test]
fn culling_hides_enclosed_blocks() {
    let mut s = Schematic::new("cube", [3, 3, 3]);
    for y in 0..3 {
        for z in 0..3 {
            for x in 0..3 {
                s.set([x, y, z], stone());
            }
        }
    }
    let placed = Placed::new(&s, Placement::new([0; 3]));
    let culled = visible_in_section(&placed, [0, 0, 0], LayerMode::All, true);
    assert_eq!(culled.len(), 26);
    assert!(!culled.iter().any(|b| b.pos == [1, 1, 1]));
    assert_eq!(
        visible_in_section(&placed, [0, 0, 0], LayerMode::All, false).len(),
        27
    );
}

#[test]
fn layers_filter_and_keep_single_layer_solid() {
    let mut s = Schematic::new("cube", [3, 3, 3]);
    for y in 0..3 {
        for z in 0..3 {
            for x in 0..3 {
                s.set([x, y, z], stone());
            }
        }
    }
    let placed = Placed::new(&s, Placement::new([0; 3]));
    // single middle layer: nothing above/below is shown, so even its centre is exposed
    let one = visible_in_section(&placed, [0, 0, 0], LayerMode::Single(1), true);
    assert_eq!(one.len(), 9);
    assert!(one.iter().all(|b| b.pos[1] == 1));
    let range = visible_in_section(
        &placed,
        [0, 0, 0],
        LayerMode::Range { min: 1, max: 2 },
        false,
    );
    assert_eq!(range.len(), 18);
}

#[test]
fn layers_follow_local_y_with_world_offset() {
    let mut s = Schematic::new("t", [1, 4, 1]);
    for y in 0..4 {
        s.set([0, y, 0], stone());
    }
    let placed = Placed::new(&s, Placement::new([5, -20, 5])); // negative world Y
    let v = visible_in_section(&placed, [0, -2, 0], LayerMode::Single(2), false);
    assert_eq!(
        v,
        vec![VisibleBlock {
            pos: [5, -18, 5],
            palette_index: placed.palette_index_at([5, -18, 5]).unwrap()
        }]
    );
}

#[test]
fn sections_cover_negative_coordinates() {
    let s = Schematic::new("big", [30, 30, 30]);
    let placed = Placed::new(&s, Placement::new([-8, -8, -8]));
    let secs = sections_overlapping(&placed);
    assert_eq!(secs.len(), 3 * 3 * 3); // -8..22 spans sections -1, 0, 1
    assert!(secs.contains(&[-1, -1, -1]) && secs.contains(&[1, 1, 1]));
    let blocks_total: usize = secs
        .iter()
        .map(|&c| visible_in_section(&placed, c, LayerMode::All, false).len())
        .sum();
    assert_eq!(blocks_total, 0); // all air
}

#[test]
fn materials_count_and_missing() {
    let mut s = Schematic::new("m", [3, 1, 1]);
    s.set([0, 0, 0], stone());
    s.set([1, 0, 0], stone());
    s.set([2, 0, 0], BlockState::new("minecraft:dirt"));
    let placed = Placed::new(&s, Placement::new([0; 3]));
    let none = material_list(&placed, None, MatchMode::Exact);
    assert_eq!(
        none[0],
        MaterialLine {
            name: "minecraft:stone".into(),
            total: 2,
            missing: 2
        }
    );
    assert_eq!(
        none[1],
        MaterialLine {
            name: "minecraft:dirt".into(),
            total: 1,
            missing: 1
        }
    );
    let mut w = HashMap::new();
    w.insert([0, 0, 0], stone());
    let world = TestWorld(w, Some(([0; 3], [3, 1, 1])));
    let some = material_list(&placed, Some(&world), MatchMode::Exact);
    assert_eq!(some[0].missing, 1);
    assert_eq!(some[1].missing, 1);
}

#[test]
fn blockstate_string_roundtrip() {
    let b = BlockState::parse("minecraft:oak_stairs[weirdo_direction=2,upside_down_bit=false]")
        .unwrap();
    assert_eq!(b.states["weirdo_direction"], StateValue::Int(2));
    assert_eq!(b.states["upside_down_bit"], StateValue::Bool(false));
    assert_eq!(BlockState::parse(&b.to_string()).unwrap(), b);
    assert!(BlockState::parse("minecraft:x[bad]").is_none());
    assert!(BlockState::parse("").is_none());
}

#[test]
fn from_raw_validates() {
    assert!(Schematic::from_raw("x", [2, 1, 1], vec![BlockState::air()], vec![0, 1]).is_err());
    assert!(Schematic::from_raw("x", [2, 1, 1], vec![BlockState::air()], vec![0]).is_err());
    assert!(Schematic::from_raw("x", [0, 1, 1], vec![BlockState::air()], vec![]).is_err());
    let ok =
        Schematic::from_raw("x", [2, 1, 1], vec![BlockState::air(), stone()], vec![0, 1]).unwrap();
    assert_eq!(ok.block_count(), 1);
}

#[test]
fn blueprint_roundtrip_keeps_typed_states_and_coordinates() {
    use schematica_core::blueprint;
    let mut source = Schematic::new("test", [2, 3, 4]);
    let block = BlockState::new("minecraft:test")
        .with("bool", StateValue::Bool(true))
        .with("byte", StateValue::Byte(-3))
        .with("int", StateValue::Int(7))
        .with("string", StateValue::Str("7".into()));
    source.set([1, 2, 3], block.clone());
    let bytes = blueprint::encode(&source).unwrap();
    let decoded = blueprint::decode(&bytes).unwrap();
    assert_eq!(decoded.size(), source.size());
    assert_eq!(decoded.get([1, 2, 3]), Some(&block));
    assert_eq!(decoded.block_count(), 1);
    assert!(blueprint::decode(&bytes[..bytes.len() - 1]).is_err());
}
