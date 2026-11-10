# Schematica for Cinnabar Builder

This workspace is based on Schematica v4.2. See the [Builder guide](../README.md) for the fork's launcher and direct import workflow.

## Install the library

Enable the imported Schematica library in **Settings > Mods** with **terrain**, **controls**, and **world_draw**. Grant **render_material** to see native block textures. Grant **settings** to remember the preview style and Target info setting.

On joining a world, the library waits for you to choose a blueprint. Press **Alt+J**, select a row, and click **Load selected**. Choosing a different row and loading it clears the previous guide before the new one appears.

| Shortcut | Action |
| --- | --- |
| M / M+C | Open the main menu / configuration |
| M+P / Numpad - | Open placement controls |
| M+L / M+V | Open the materials table / verifier |
| M+R / M+G | Toggle all schematic rendering / only the ghost blocks |
| Page Up/Down / M+Page Up/Down | Change the selected layer / change the layer mode |
| Alt+J | Open the blueprint library |
| Alt+I | Open manipulation controls and the materials table |
| Alt+K | Open preview and placement settings |
| Alt+U | Identify the planned block you are aiming at |
| Alt+P | Re-anchor the active guide ahead of you |
| Alt+arrow keys / Alt+Page Up or Down | Nudge along X/Z or Y |
| Alt+R / Alt+M | Rotate clockwise / cycle horizontal mirror |
| Alt+L / Alt+[ / Alt+] | Cycle layer modes / adjust the layer |
| Alt+B / Alt+H | Switch preview style / toggle visibility |

Cinnabar keeps its regular chat, crouch, and pick-block controls. This release uses Alt chords so the slash keys, Numpad *, and Shift+middle-click can keep their normal Cinnabar behavior.

Use **Hotkeys** in the main menu to change the Litematica-style bindings. Enter a key such as `F6` or a chord such as `M+V` and press Enter. A blank field unbinds that action. The placement lock prevents edits to the active guide. The verifier lists coordinates from the currently scanned layers and distinguishes missing blocks, wrong blocks, wrong states, extra blocks, and unknown cells.

## Preview and placement settings

- **Block textures** uses Cinnabar's native block models and textures with adjustable opacity on Builder, starting at 65%. Stock Cinnabar uses the blocks' native opacity. **Hidden** turns off the missing-block preview. Yellow and red still mark wrong and extra blocks. [Cinnabar drawing contract](https://cinnabar.gg/docs/modding/reference/world-draw-drawing)
- **Target info** shows the expected material and coordinates under your aim.
- **Fast Place** requires the matching Builder fork and the Inventory and Item Use grants. It selects or requests a Creative item for one right-click on a supported missing block. See the [Builder guide](../README.md).

## Create a library with your own files

Extract **schematica-import-v4.2-windows-x64.zip**. Keep the files together and open **Import-Schematics.cmd**. Use Ctrl-click to choose up to eight **.schematic**, **.schem**, **.litematic**, or **.mcstructure** files in one folder. It writes a single **schematica-library.cinnabar.wasm** beside a multi-file selection. A single-file selection produces **your-build.cinnabar.wasm**. Move an existing output file before converting again.

For files in different folders, use PowerShell.

```powershell
.\Import-Schematics.ps1 -Output 'C:\path\library.wasm' -InputFiles 'C:\a\one.schem','C:\b\two.schematic'
```

Each blueprint supports up to 64 blocks on each axis, 5,120 cells including air, and 128 palette entries. Translation uses Nucleation's Geyser-derived block mappings. Complex states may differ after translation or rotation. World capture and printer automation remain unavailable.

## Build from source

```powershell
rustup target add wasm32-unknown-unknown
cargo test --locked --workspace
.\Build-Schematica.ps1 'C:\path\to\your-build.schematic'
```

For the native importer source, build `schematica-builder` for Windows and `schematica-guest` for `wasm32-unknown-unknown`. Place the compiled guest next to the executable as `schematica-guest-template.bin`. The guest's JSON-UI assets are checked into `guest/ui`. Regenerate them after editing the layout with `.\guest\menu_layout.ps1 using PowerShell 7`.

The guest targets Cinnabar's extension 0.1.0 and world-draw 0.2.0 interfaces. The WIT snapshot is from [Cinnabar](https://github.com/cinnabar-mc/cinnabar) under the license in `guest/wit/LICENSE`. The layout reference is [Lunatrius Schematica](https://github.com/Lunatrius/Schematica), whose MIT license is included in `SCHEMATICA-LICENSE.txt`.



