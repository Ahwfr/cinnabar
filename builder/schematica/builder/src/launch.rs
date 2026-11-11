//! Portable Builder entry point and lossless library upgrades.

use anyhow::{Context, Result, ensure};
use schematica_core::blueprint;
use std::{
    env, fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// Resolves the matching template supplied by this release, never another checkout.
pub fn template() -> Result<Vec<u8>> {
    let path = env::var_os("SCHEMATICA_GUEST_TEMPLATE")
        .map(PathBuf::from)
        .unwrap_or(env::current_exe()?.with_file_name("schematica-guest-template.bin"));
    fs::read(&path).with_context(|| format!("Read the guest template at {}", path.display()))
}

/// Copies the old library payload into this release's guest, retaining every imported blueprint.
pub fn refresh(component: &[u8]) -> Result<Vec<u8>> {
    let template = template()?;
    refresh_with_template(component, &template)
}

fn refresh_with_template(component: &[u8], template: &[u8]) -> Result<Vec<u8>> {
    let payload = library_payload(component)?;
    super::component(&super::populate_template(template, &payload)?)
}

fn library_payload(component: &[u8]) -> Result<Vec<u8>> {
    let mut found = None;
    for section in wasmparser::Parser::new(0).parse_all(component) {
        if let wasmparser::Payload::DataSection(data) = section? {
            for segment in data {
                let segment = segment?;
                for slot in segment.data.windows(blueprint::SLOT_BYTES) {
                    if let Some(payload) = blueprint::embedded_payload(slot) {
                        blueprint::library(payload).map_err(anyhow::Error::msg)?;
                        ensure!(
                            found.is_none(),
                            "The old component contains ambiguous blueprint slots"
                        );
                        found = Some(payload.to_vec());
                    }
                }
            }
        }
    }
    found.context(
        "The old Schematica library has no supported embedded blueprint slot. Import its original files in game."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_encoder::{Component, ConstExpr, DataSection, Module, ModuleSection};

    #[test]
    fn nested_component_keeps_the_entire_imported_library_payload() -> Result<()> {
        let first = super::super::encode_blueprint(&super::super::demo())?;
        let payload = blueprint::encode_library(&[first.clone(), first]).unwrap();
        let slot = blueprint::embedded_slot(&payload).unwrap();
        let mut data = DataSection::new();
        data.active(0, &ConstExpr::i32_const(0), slot);
        let mut core = Module::new();
        core.section(&data);
        let mut component = Component::new();
        component.section(&ModuleSection(&core));
        assert_eq!(library_payload(&component.finish())?, payload);
        Ok(())
    }
}

/// Publishes only complete components and first preserves the previous revision.
fn publish(mods: &Path, bytes: &[u8]) -> Result<()> {
    fs::create_dir_all(mods)?;
    let target = mods.join("schematica-builder.wasm");
    if target.is_file() && fs::read(&target)? == bytes {
        return Ok(());
    }
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let staged = mods.join(format!(
        ".schematica-{}-{nonce}.pending",
        std::process::id()
    ));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staged)?;
    use std::io::Write;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    if target.is_file() {
        let backup = mods
            .parent()
            .context("mods data folder")?
            .join("schematic-backups");
        fs::create_dir_all(&backup)?;
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        fs::copy(&target, backup.join(format!("{stamp}.wasm")))?;
    }
    fs::rename(&staged, &target).context("Publish the complete schematic library")?;
    Ok(())
}

#[cfg(windows)]
pub fn import_dialog(mods: &Path) -> Result<()> {
    let Some(inputs) = super::picker::choose()? else {
        return Ok(());
    };
    let staged = mods.with_file_name(format!(".schematica-import-{}.wasm", std::process::id()));
    super::write_library(&inputs, &staged, false)?;
    let bytes = fs::read(&staged)?;
    publish(mods, &bytes)?;
    fs::remove_file(&staged)?;
    println!("SCHEMATICA_IMPORTED");
    Ok(())
}

#[cfg(windows)]
pub fn start(root: &Path) -> Result<()> {
    let client = root.join("runtime/cinnabar.exe");
    ensure!(
        client.is_file(),
        "Keep Cinnabar Builder.exe with the runtime folder from the same download."
    );
    let template_path = root.join("schematica-guest-template.bin");
    let template =
        fs::read(&template_path).context("Read this release's Schematica guest template")?;
    let revision = fs::read_to_string(root.join("builder-commit.txt"))?;
    ensure!(
        super::guest_revision(&template)? == Some(revision.trim()),
        "The launcher package and guest template belong to different builds. Extract the complete release ZIP."
    );
    let profile = env::var_os("CINNABAR_USER_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("profile"));
    fs::create_dir_all(&profile)?;
    let profile = profile.canonicalize()?;
    let mods = profile.join("data/mods");
    let installed = mods.join("schematica-builder.wasm");
    let bytes = if installed.is_file() {
        refresh_with_template(&fs::read(&installed)?, &template)?
    } else {
        super::component(&template)?
    };
    publish(&mods, &bytes)?;
    registration(&profile, &installed)?;
    std::process::Command::new(client)
        .current_dir(root.join("runtime"))
        .env("CINNABAR_USER_ROOT", &profile)
        .env("CINNABAR_SCHEMATICA_IMPORTER", env::current_exe()?)
        .env("SCHEMATICA_GUEST_TEMPLATE", template_path)
        .env("CINNABAR_UPDATE_CHECK", "0")
        .spawn()
        .context("Start Cinnabar Builder")?;
    Ok(())
}

/// Keeps the existing grants and enabled state while relocating this library's registration.
#[cfg(windows)]
fn registration(profile: &Path, installed: &Path) -> Result<()> {
    let directory = profile.join("config");
    fs::create_dir_all(&directory)?;
    let path = directory.join("local-mod.json");
    let mut document: serde_json::Value = if path.is_file() {
        serde_json::from_slice(&fs::read(&path)?)?
    } else {
        serde_json::json!({"version":2,"mods":[]})
    };
    let mods = document["mods"]
        .as_array_mut()
        .context("Invalid mod registration list")?;
    if let Some(entry) = mods.iter_mut().find(|entry| {
        entry["component"]
            .as_str()
            .and_then(|name| Path::new(name).file_name())
            == Some(std::ffi::OsStr::new("schematica-builder.wasm"))
    }) {
        entry["component"] = serde_json::json!(installed);
    } else {
        let mut grants: serde_json::Value =
            serde_json::from_slice(include_bytes!("../../guest/grants.json"))?;
        grants["commands"] = serde_json::json!([]);
        mods.push(serde_json::json!({
            "component":installed,"enabled":true,"grants":grants,"reload":0
        }));
    }
    let staged = path.with_extension("next");
    fs::write(&staged, serde_json::to_vec(&document)?)?;
    fs::rename(staged, path)?;
    Ok(())
}
