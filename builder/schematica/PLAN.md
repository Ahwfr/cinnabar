# Builder implementation and acceptance plan

The fork keeps the upstream runtime and adds generic native capabilities used by the Schematica component.

| Feature | Current implementation | Acceptance |
| --- | --- | --- |
| Creative supplies | Confirmed background inventory window, catalog take to cursor, hotbar transfer, close, select and native placement | User confirmed Creative pickup and hotbar selection on cf3cd54 |
| Carried inventory | Main-inventory moves now open the server inventory and wait for confirmation before using the regular hotbar swap | Needs a new live Creative and Survival check |
| Orientation | Optional server-facing aim, bounded to four input ticks, with real support ray and predicted state checks. Camera pose stays local | Needs representative stairs/slabs/logs against the server |
| State mismatch | Matching block identifiers may place in their native orientation even if the planned state differs | Regression fixture updated. Native server acceptance still applies |
| Floating cells | Optional setblock keep command on a physical use press over a known missing ghost, with no native block target | Needs operator/cheats and the setblock grant. Does not consume inventory. Ordinary Survival air placement remains unsupported by the native item-use path |
| Import | In-game Mods page runs the matching bundled converter on its worker and reloads enabled components | Needs native picker check from the packaged runtime |
| Guest upgrades | Portable launcher copies the old library payload into the new template, backs up the old component, and stamps the name and metadata version | Nested payload preservation regression and actual library smoke check |
| UI | Minecraft-style gray/green panels, button feedback, full viewport or compact mode | Requires rendered checks at small and large window sizes |
| Bindings | Schematica has normal Keyboard & Mouse rows. The guest observes their physical assignments | Remap and unbind regression. Legacy Alt+J/I/K remain |
| Packaging | One Cinnabar Builder.exe entry point beside runtime, template and persistent profile. Includes asset preparation kit and exact source ZIP | Needs extracted package startup check |

The UI uses the existing JSON-UI mod surface with Minecraft-style controls. Import and binding edits use native OreUI Settings. Every release keeps the same mod filename so its enabled state, permissions and settings can follow the upgraded code.

Remaining upstream parity work includes large schematic streaming, capture/export, complete Java-to-Bedrock state mappings and multi-block placement fixtures. Current import bounds remain documented in README. A successful compilation does not establish server placement acceptance.
