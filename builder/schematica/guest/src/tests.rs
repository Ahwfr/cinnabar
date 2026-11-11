use super::*;

#[test]
fn menu_edits_turn_the_guide_and_select_layer_ranges() {
    let mut viewer = Viewer::new(
        Schematic::new("fixture", [2, 4, 3]),
        Vec::new(),
        vec!["Demo".into(), "Wagon".into()],
        Options {
            textured: true,
            assist: false,
            fast_place: false,
            correct_aim: false,
            fullscreen_ui: true,
            air_place: false,
            opacity: 65,
            hotkeys: BTreeMap::new(),
        },
        true,
    );
    viewer.placement = Some(Placement::new([12, 64, -5]));
    assert!(viewer.options.textured);
    viewer.menu_action("select-1");
    viewer.menu_action("select-8");
    assert_eq!(viewer.selected, Some(1));

    viewer.menu_action("ccw");
    let placement = viewer.placement.unwrap();
    assert_eq!(placement.quarter_turns, 3);
    assert_eq!(placement.footprint(viewer.schematic.size()), [3, 4, 2]);
    assert!(viewer.dirty);
    viewer.menu_action("cw");
    assert_eq!(viewer.placement.unwrap().quarter_turns, 0);

    viewer.menu_action("mode");
    viewer.menu_action("next-layer");
    assert_eq!(viewer.layers(), LayerMode::Single(1));
    viewer.menu_action("mode");
    assert_eq!(viewer.layers(), LayerMode::Range { min: 0, max: 1 });
    viewer.menu_action("mode");
    assert_eq!(viewer.layers(), LayerMode::All);
}
