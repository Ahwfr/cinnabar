# Cinnabar Builder

Extract the whole ZIP into a folder you can write to. Open **Cinnabar Builder.exe**.
Keep the **App/** folder beside it. It contains the required runtime, importer template and documentation. The first launch prepares Minecraft assets.

Your worlds, login, options and schematic library live in **profile/**.
For an update, close Builder and extract the new ZIP into the same folder.
Keep profile/. The launcher upgrades the library to the new build and backs up its old component.
Both the Mods page and the Schematica header show the guest build revision.

Press **M** for the schematic menu. **Import schematics** opens Settings → Mods.
Press **Import schematics…**, choose your Java or Bedrock files, and return to the world.
Enabled libraries reload automatically. Each import replaces the library selection and backs up the previous component.
Original input files stay at their original paths.
The red **Delete** button removes a schematic from the library and clears its active guide.
Original files and recovery backups are preserved. Reimport to restore a removed entry.
Materials show the native inventory artwork: isometric block textures or item sprites as appropriate.

Edit the Schematica bindings in **Settings → Keyboard & Mouse**.
Defaults are M for Menu, F6 for Settings, F7 for Placement, F8 for Verifier,
F9 for Materials, F10 for Rendering, Numpad 0 for Ghosts, Numpad +/* for layer modes,
Numpad - for placement settings, and Page Up/Down for layers.
The old Alt+J/I/K shortcuts remain available.

Turn **Fast Place** on to select carried blocks, move main-inventory blocks to the hotbar,
or obtain a Creative catalog block. Right-click a real support beside the ghost.
**Auto orientation** optionally adjusts the reported aim without moving your camera.
If a valid aim cannot produce the exact state, ordinary placement of the matching block can still proceed.

**Air placement** is a separate operator command mode, off by default.
It sends one /setblock with the planned states and keep handling when you right-click a missing floating ghost
with no real block under the crosshair. It needs the setblock mod grant and server cheats/command permission.
Commands do not consume inventory items. Inventory-based Survival placement requires a real support.

Choose **Menu layout** in Schematica Settings for full screen or compact mode.
Builder-Source.zip contains the editable fork source for this exact build.
