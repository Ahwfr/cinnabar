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

On the fork's [Actions page](https://github.com/Ahwfr/cinnabar/actions), open a successful **Builder Windows** run for the commit you checked out. Download **cinnabar-builder-windows-x64**. Extract its `target` folder into the checkout root with Builder closed. The launcher checks the build's commit when it starts.

The artifact contains the binaries and notices. The checkout needs its locally generated game assets. If you haven't prepared them yet, run `target/release/assetc.exe prepare --accept-eula` from the checkout root once you accept the Minecraft EULA. Then open **Play Builder.cmd**.

## Import a schematic

Close Builder and open **Import Schematics.cmd** in the same folder. Choose up to eight `.schematic`, `.schem`, `.litematic` or `.mcstructure` files. The importer builds one library directly in this Builder profile. It leaves your input files in place and keeps the previous library under `.local/builder-user/library-backups`. If conversion fails, the previous library remains installed.

Start Builder and enable **schematica-builder** under **Settings → Mods**. Grant the mod's requested drawing and terrain permissions. In a world press **Alt+J**, choose the schematic and select **Load selected**. The importer currently limits each schematic to 64 blocks on an axis, 5,120 cells including air, and 128 palette entries.

You can also pass exact files, or install the built-in arch to try the flow:

```powershell
.\builder\Start-Builder.ps1 -ImportOnly -InputFiles 'C:\Builds\station.schem','C:\Builds\wagon.schematic'
.\builder\Start-Builder.ps1 -ImportOnly -Demo
```

Target info identifies the planned material under the crosshair. Creative Easy Place remains on the roadmap.

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
