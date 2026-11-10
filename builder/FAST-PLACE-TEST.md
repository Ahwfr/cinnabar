# Fast Place diagnosis on Bedrock 1.26.60

The installed session used client `e92eeb2` and local Bedrock Dedicated Server `1.26.60.6` (wire protocol 2225). Fast Place was enabled. That session logged no assisted placement attempts. It did not establish whether a particular click reached the assist.

`GameplayContext::block_hit` previously required an admitted hand selection. Queuing an equipment announcement can make that selection temporarily unavailable. That would hide the placement cell from the mod, withdraw its offer, and make `creative::resume` cancel its own in-flight action. The target query now reads geometry independently of the announcement. The transaction still requires the confirmed slot and a fresh native support hit.

## Trace one click

Close Builder and run these commands in the updated checkout:

```powershell
$env:CINNABAR_LOG = 'info,cinnabar::block_use=debug,cinnabar::inventory_requests=debug,cinnabar::placement_updates=debug'
.\builder\Start-Builder.ps1
```

Check `.local/builder-user/data/logs/client.log` for the session revision, then check **Settings → Mods** for the imported guest revision. Open a disposable local world, enable Fast Place in Schematica settings, and grant terrain, item use and inventory. Place a real support bordering one missing ghost. Click once and keep the crosshair still for one second.

- **Survival.** Start with an empty selected slot and two matching blocks in the main inventory. Expect a request ID, an accepted inventory response for that ID, a ready backing stack, equipment admission, and one click-block batch. The placed item count should drop by one. Repeat with the matching stack in a different hotbar slot. That path has no inventory request.
- **Creative.** Start with no matching item and one empty hotbar slot. Expect an inventory open, a catalog take to cursor, a transfer to the hotbar, and an inventory close before equipment admission and one click-block batch. Both inventory requests should be accepted. The item should remain in the selected slot.
- **Manual.** Hold the planned block and click its support with Fast Place enabled. An exact placement goes through the assist, and a mismatched orientation is refused. Turn Fast Place off to place freely. Repeat while standing inside the hologram and aiming outside the schematic. The normal placement path owns that click.

Run the Survival and Creative checks on plain stone before checking these states:

| Planned block | Aim at a real support that can produce this state |
| --- | --- |
| Bottom and top slabs | For the same side face, click below or above its midpoint. Test with separate empty destination cells. |
| Stairs | Use a straight stair, align your facing with the blueprint, then click the appropriate half of the support face. Repeat for an upside-down stair. |
| Logs | Click the top face for a vertical log. Use an east/west side for X axis or north/south side for Z axis. |
| Furnace | Turn to the facing that would produce the required orientation with a normal click. Test a matching and a deliberately different direction. |

Use the Verifier to check the resulting state, then leave and rejoin the disposable world to verify persistence. The client locally predicts placement after transport admission. A matching server block update is stronger evidence than the predicted render, but the absence of an update does not by itself prove rejection.

## Read the result

- **No native support hit:** the real click did not have a placeable support in reach. **Native hit has no schematic offer:** the placement cell had no active blueprint block or the mod did not publish it. Check the active layer, Fast Place setting, and Mods permissions.
- **Request refused locally:** the request did not leave the client. **Inventory response rejected:** the server declined that request. **Inventory did not confirm the requested block:** no matching authoritative backing stack appeared after the request retired.
- **Offer changed before placement:** the current schematic offer disappeared or changed while the press was pending. **Support changed:** the resumed native hit no longer matches the click that started the request.
- **Predicted a different block or state:** no assisted placement was queued. The line includes expected and actual state, click face and offset, and the yaw/pitch used by prediction. Cinnabar's current assist does not choose and publish a replacement rotation or try an alternate face. Multi-cell placement is still excluded from the assist.
- **Placement batch admission sent=true:** the native batch entered the transport queue. Its click uses the ordinary `InventoryTransaction` path, while movement continues through `PlayerAuthInput`. Compare the destination and predicted wire runtime ID with the received block-update line. A different runtime ID at the same position can identify a correction or a server-computed state. There is no automatic retry.

For a failure, send the session start line, the mode and block tested, and the lines from the first use press through the next server update. The relevant prefixes are `Fast Place`, `cinnabar::inventory_requests` and `cinnabar::placement_updates`.
