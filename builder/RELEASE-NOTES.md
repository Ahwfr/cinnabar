# Cinnabar Builder 0.3.0

- Added a portable Cinnabar Builder.exe entry point with a persistent profile and first-run asset preparation.
- Grouped support files under App so the play folder has one obvious executable.
- Added in-game schematic importing through the native Mods page, with library backups and enabled-mod reload.
- Added a browser layout with a schematic list, placement controls, build details and material preview.
- Added beveled buttons with eased hover feedback and a depressed pressed state.
- Fixed browser button text padding and sizing, including hover and pressed labels.
- Added Delete on every library row. Removals persist for that import, preserve original files, and reset on reimport.
- Added native textured inventory icons to the dashboard and material list, including isometric artwork for blocks.
- Added full screen and compact layouts, authored outside the WASM callback budget.
- Added Schematica rows to the normal Keyboard & Mouse settings.
- Added a shared release version and guest revision in the mod name, metadata and menu.
- Updated carried-inventory transfer to wait for a confirmed background inventory window.
- Removed the mandatory exact-orientation placement restriction.
- Retained optional server-facing orientation assistance without moving the camera.
- Added optional keep-only command placement for floating cells.

Air command placement needs server cheats/operator permission and the setblock mod grant. It does not consume inventory items. Ordinary Survival placement needs a real support. Orientation correction remains subject to server acceptance.

Focused importer, library, inventory and binding tests pass. Client test targets compile. Rendered UI checks cover compact/full screen layouts and button interaction states. Live server placement and the packaged import dialog still need confirmation on this release.
