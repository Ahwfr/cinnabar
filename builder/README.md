# Cinnabar Builder

Builder keeps the Cinnabar launcher and uses an isolated profile. Schematica's textured previews use 65% opacity when hosted by this fork. The mod continues to work in stock Cinnabar using its native block opacity.

## Build and launch on Windows

Check out `builder-features` from the fork. Follow the main [Cinnabar README](../README.md) for the native build requirements. Install Rust via rustup, Go, and GNU Make. Make needs a shell and Unix utilities such as `awk` on PATH on Windows, as provided by Git for Windows or MSYS2. The checkout pins the Rust toolchain and the build uses the upstream `make play-build` target.

```powershell
git clone --branch builder-features https://github.com/Ahwfr/cinnabar.git cinnabar-builder
Set-Location cinnabar-builder
.\builder\Build-Builder.ps1
.\builder\Start-Builder.ps1 -Check
```

You can also open **Build Builder.cmd** in the `builder` folder. When the build finishes, open **Play Builder.cmd** to start the client. Its account, worlds, settings and mods live under `.local/builder-user` in this checkout. The shortcuts use Windows PowerShell with a process-only `RemoteSigned` policy for the scripts checked out by Git.

### Use a GitHub build

On the fork's [Actions page](https://github.com/Ahwfr/cinnabar/actions), download the successful **Builder Windows** artifact. Extract the whole ZIP and open **Cinnabar Builder.exe** at its root. The first launch prepares Minecraft assets. Keep **profile/** when extracting updates into the same folder. The launcher upgrades an existing schematic library and backs up its previous component. See [the portable guide](PORTABLE-README.md).
Builder Windows saves Cargo's compiled dependencies and incremental compiler data between runs. A feature or fix can reuse work from the previous successful build. Cargo checks which inputs changed and recompiles what the new executable needs. The first run with a new cache or Rust toolchain has more work to do. The workflow builds the runtime and schematic importer and publishes a complete ZIP.

## Import a schematic

Open **Schematica → Import schematics**. On the native Mods page, press **Import schematics…** and choose up to eight .schematic, .schem, .litematic or .mcstructure files. Conversion runs off the render thread. Successful imports replace the selection, back up the old component and reload enabled mods. Original input files remain in place. The checkout's **Import Schematics.cmd** also remains available.

The mod name **Schematica Builder [commit]**, metadata version and menu header identify the guest code. The launcher updates the code while retaining the library payload. Grant requested permissions in Mods. Press **M** for the menu or **Alt+J** for the library, choose a schematic and select **Load selected**. Normal **Settings → Keyboard & Mouse** contains the Schematica bindings. The current import limits are 64 blocks on an axis, 5,120 cells including air, and 128 palette entries.
You can also pass exact files, or install the built-in arch to try the flow:

```powershell
.\builder\Start-Builder.ps1 -ImportOnly -InputFiles 'C:\Builds\station.schem','C:\Builds\wagon.schematic'
.\builder\Start-Builder.ps1 -ImportOnly -Demo
```

**Fast Place** is off by default. In a Creative or Survival world, open **Alt+K** and turn it on. Grant **Inventory** and **Item Use** in Settings → Mods if requested. Aim at the face of a real block bordering a missing schematic cell and right-click once. The clicked face determines the placement cell even if you are standing inside the schematic. Builder first selects a matching hotbar block, or moves a matching main-inventory stack to a free hotbar slot (swapping with the selected slot if full). Survival consumes your blocks normally and never requests catalog items. Creative can use the normal catalog take and hotbar transfer in a background inventory window when an empty hotbar slot is available. Placement waits for inventory confirmation. Leave a hotbar slot free if you do not already carry the block. Fast Place permits a different orientation of the same block. Turn off Fast Place to place freely. Clicks outside the schematic still place normally. The assist uses the actual clicked face and view direction, and requires a valid native placement. The portable launcher upgrades the library controls when you update. The client log records why an offered placement was refused.

**Auto orientation** is a separate Schematica setting and starts off. With both settings enabled, stand still and right-click a real face adjoining the ghost. If your aim would give the block the wrong state, Builder looks for an unobstructed aim in reach that yields the exact state. The correction is sent to the server for that placement while your camera keeps its current direction. Leave it off on servers that disallow it.

## Update for a new Bedrock version

Wait for upstream Cinnabar to add support for the version. From a clean `builder-features` checkout run:

```powershell
& '.\builder\Update Builder.cmd'
```

The script fetches upstream `dev` and replays the Builder commits in a sibling worktree ending in `-update`. The main checkout, build and profile stay available. If Git stops on a conflict, resolve it in the update worktree and run `git rebase --continue`, or run `git rebase --abort` there to abandon the attempt. Git may need you to deepen a shallow clone before it can find a shared base.

From the update worktree run `./builder/Build-Builder.ps1` and launch it with `./builder/Start-Builder.ps1`. The launcher's `-Check` switch shows the Bedrock version from Cinnabar's manifest. Use the isolated test profile to import the demo and a Java schematic, then check textured preview, layers, and representative mapped block states. Check that the client connects to the intended Bedrock version. Then return to the main checkout:

```powershell
& '.\builder\Update Builder.cmd' -Accept
& '.\builder\Build Builder.cmd'
```

The script advances the branch and base together and saves the previous commit on a timestamped `builder-backup-*` branch. The main launcher checks the build's commit before starting. After the new build works, remove the clean candidate using `git worktree remove <update-worktree-path>` and `git branch -d builder-update` to prepare for the next update. If you want to retain the candidate build and its separate profile, detach the candidate and use `git worktree move` to archive it under a different path before deleting the trial branch.
