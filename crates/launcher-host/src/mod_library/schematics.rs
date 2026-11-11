//! Runs the bundled converter on the library worker, away from the rendering thread.

use std::{path::Path, process::Command};

/// Opens the converter's native picker and waits for its atomic library publication.
pub(super) fn import(mods: &Path) -> Result<bool, String> {
    let helper = std::env::var_os("CINNABAR_SCHEMATICA_IMPORTER").ok_or(
        "Launch Cinnabar Builder.exe from the complete Builder download to import schematics.",
    )?;
    let mut command = Command::new(helper);
    command.arg("import-dialog").arg(mods);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let result = command
        .output()
        .map_err(|error| format!("Could not start the schematic importer: {error}"))?;
    if !result.status.success() {
        return Err(format!(
            "Schematic import failed. {}",
            String::from_utf8_lossy(&result.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&result.stdout).contains("SCHEMATICA_IMPORTED"))
}
