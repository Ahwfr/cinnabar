# Schematica parity plan

Original Schematica combines a projected build, material list, in-game capture, and assisted placement. This project implements the projected-build workflow as a Cinnabar personal mod, with file preparation on the desktop. [Schematica](https://github.com/Lunatrius/Schematica) · [Cinnabar personal mods](https://github.com/cinnabar-mc/cinnabar/blob/dev/docs/modding/src/appendix/personal-mods.md)

| Feature | Implementation |
| --- | --- |
| Import | The host-side builder converts Java formats using Nucleation's Geyser-derived mappings and reads the primary palette of Bedrock structures directly. The Windows picker imports a file and embeds one blueprint in a ready-to-load component. |
| Preview | The guest reads terrain in small tiles and draws missing blocks as translucent cyan geometry by default. The textured mode uses Cinnabar's block mesher. Colored markers identify wrong, extra, and unknown cells. |
| Placement | Anchor from the camera, nudge, rotate and mirror with the existing core. A local-Y filter selects a layer or a range. The in-game menu exposes both rotation directions. |
| Materials | The in-game panel shows a paged table of total and still-needed block counts. |
| Capture and export | Terrain read access supplies the block data. An export workflow needs a way to deliver the result from the sandbox to the desktop. |
| Printer | Pending. A candidate design needs to test Cinnabar's input and interaction gates, valid target faces, orientation, inventory selection and server acceptance. |

The current viewer caps blueprint volume to keep worst-case marker geometry within the retained drawing budget. Terrain samples can arrive later than the callback that requested them. The viewer distinguishes unknown cells from loaded air, skips redraws when classifications are unchanged, and releases retired handles over several frames to respect the host's 32 drawing calls per callback. [Terrain API](https://github.com/cinnabar-mc/cinnabar/blob/dev/docs/modding/src/appendix/terrain.md) · [Block geometry API](https://github.com/cinnabar-mc/cinnabar/blob/dev/docs/modding/src/appendix/block-geometry.md)

The next parity milestone is a live client check of the updated menu, followed by representative fixtures for slabs, stairs, doors and waterlogged blocks against the target registry. Larger builds need nearby-tile streaming to keep retained geometry bounded. A future blueprint library would need a supported host route for choosing multiple desktop files or a manifest of bundled blueprints.


V4 embeds up to eight independently decoded blueprints in a single personal mod. Only an explicitly chosen blueprint renders. The centered panels use bounded local offsets, the preview defaults to native textures, and the optional Placement assist labels the aimed planned block. Stock Cinnabar still needs dedicated native APIs for textured opacity, targeted placement, and hotbar selection.


## Native client capabilities needed for the requested behavior

- Creative right-click placement needs a host request that resolves a block state to a Creative item, selects an admitted hotbar slot through the inventory ledger, and submits one placement through the normal use path after selection settles. It needs the clicked face and the predicted resulting block state. The existing Creative-take import chooses the first empty inventory slot or cursor, and the use pulse acts on the held item and normal crosshair.
- True textured translucency needs a host-side alpha parameter and a block draw pass that blends with the world without writing ordinary opaque depth. Stock retained block grids require white tint, and the additive blend brightens the destination rather than compositing the original texture at partial opacity.
