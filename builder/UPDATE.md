# Installing and updating Builder

Download the Windows ZIP from a versioned GitHub Release. Extract every file into a writable folder and open **Cinnabar Builder.exe**.

For the next release, close Builder and extract the new package into the same folder. Keep **profile/**. It contains your worlds, account data, settings, mods and library. The launcher checks that the template matches the package revision, upgrades the library code and backs up the previous component. Confirm the release version and guest hash in the Schematica header or Mods page.

Import schematics from Schematica's Import button, then **Import schematics…** on the native Mods page. A new import replaces the current library selection and preserves its previous component. Original input files remain in place.

Only the Windows package, exact source ZIP and documentation belong in a release. Profiles and compiler caches stay on your computer. The archive ZIP can be removed from your computer after extraction if you keep another backup.

# Maintaining the fork

The shared version is in **builder/schematica/Cargo.toml**, under **workspace.package**. Increment the patch for a fix and the minor version for a feature update. Update RELEASE-NOTES.md for the same version. The guest and importer read this version automatically. Release tags use **builder-vVERSION**.

Use the existing editable checkout and one persistent Cargo build cache for subsequent builds. The **play** profile enables incremental compilation and omits debug information. UI-only changes need a guest/importer rebuild, and can use an unchanged native runtime only when its API matches.

The Windows workflow packages successful builds and publishes a versioned release. GitHub can pause hosted builds on a fork. A local build remains available while a maintainer resolves that restriction.

To publish an already staged and verified package, run **builder/Publish-BuilderRelease.ps1 -PackageDirectory PATH -Publish**. It excludes the private profile, editable source folder and compiler caches from the ZIP. It refuses to overwrite an existing version tag.
