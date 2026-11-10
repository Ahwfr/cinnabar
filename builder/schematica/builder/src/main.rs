mod import;
#[cfg(windows)]
mod picker;

use anyhow::{Context, Result, bail, ensure};
use schematica_core::{BlockState, Schematic, blueprint};
use std::{
    borrow::Cow,
    env, fs,
    io::Write,
    path::{Path, PathBuf},
};
use wasm_encoder::{CustomSection, Encode};

fn main() -> Result<()> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    match args.as_slice() {
        #[cfg(windows)]
        [] => {
            picker::run();
            Ok(())
        }
        [cmd, input] if cmd == "convert" => {
            let output = convert(Path::new(input))?;
            println!("Wrote {}", output.display());
            Ok(())
        }
        [cmd, output, inputs @ ..] if cmd == "library" || cmd == "demo-library" => {
            let paths: Vec<_> = inputs.iter().map(PathBuf::from).collect();
            write_library(&paths, Path::new(output), cmd == "demo-library")?;
            println!("Wrote {}", Path::new(output).display());
            Ok(())
        }
        [cmd, output] if cmd == "demo" => write_blueprint(&demo(), Path::new(output)),
        [cmd, input, output] if cmd == "import" => {
            let input = Path::new(input);
            let out =
                import::import(input).with_context(|| format!("import {}", input.display()))?;
            write_blueprint(&out, Path::new(output))
        }
        [cmd, input, output] if cmd == "pack" => {
            let core = fs::read(input)?;
            fs::write(output, component(&core)?)?;
            println!("Wrote Cinnabar component {}", Path::new(output).display());
            Ok(())
        }
        [cmd, template, input, output] if cmd == "pack-blueprint" => {
            let core = populate_template(&fs::read(template)?, &fs::read(input)?)?;
            fs::write(output, component(&core)?)?;
            println!("Wrote Cinnabar component {}", Path::new(output).display());
            Ok(())
        }
        _ => bail!(
            "usage: schematica-builder convert INPUT | library OUTPUT.wasm INPUT... | demo OUTPUT.scb | import INPUT OUTPUT.scb | pack GUEST.wasm OUTPUT.wasm"
        ),
    }
}

fn convert(input: &Path) -> Result<PathBuf> {
    convert_selection(&[input.to_path_buf()])
}

fn convert_selection(inputs: &[PathBuf]) -> Result<PathBuf> {
    let first = inputs.first().context("choose at least one schematic")?;
    let output = if inputs.len() == 1 {
        first.with_extension("cinnabar.wasm")
    } else {
        first.with_file_name("schematica-library.cinnabar.wasm")
    };
    write_library(inputs, &output, false)?;
    Ok(output)
}

fn write_library(inputs: &[PathBuf], output: &Path, include_demo: bool) -> Result<()> {
    let count = inputs.len() + usize::from(include_demo);
    ensure!(
        (1..=blueprint::MAX_LIBRARY).contains(&count),
        "choose 1–{} schematics",
        blueprint::MAX_LIBRARY
    );
    let template_path = env::var_os("SCHEMATICA_GUEST_TEMPLATE")
        .map(PathBuf::from)
        .unwrap_or(env::current_exe()?.with_file_name("schematica-guest-template.bin"));
    let template = fs::read(&template_path).with_context(|| {
        format!(
            "read the matching guest template at {}",
            template_path.display()
        )
    })?;
    let mut entries = Vec::with_capacity(count);
    if include_demo {
        entries.push(encode_blueprint(&demo())?);
    }
    for input in inputs {
        let schematic =
            import::import(input).with_context(|| format!("import {}", input.display()))?;
        entries.push(encode_blueprint(&schematic)?);
    }
    let payload = blueprint::encode_library(&entries).map_err(anyhow::Error::msg)?;
    let core = populate_template(&template, &payload)?;
    let component = component(&core)?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .with_context(|| {
            format!(
                "create {} (choose another filename if this output already exists)",
                output.display()
            )
        })?;
    file.write_all(&component)?;
    Ok(())
}

fn populate_template(template: &[u8], payload: &[u8]) -> Result<Vec<u8>> {
    let empty = blueprint::embedded_slot(&[]).map_err(anyhow::Error::msg)?;
    let mut offset = None;
    for section in wasmparser::Parser::new(0).parse_all(template) {
        if let wasmparser::Payload::DataSection(data) = section? {
            for segment in data {
                let segment = segment?;
                if !matches!(segment.kind, wasmparser::DataKind::Active { .. }) {
                    continue;
                }
                for (index, _) in
                    segment
                        .data
                        .windows(empty.len())
                        .enumerate()
                        .filter(|(_, bytes)| {
                            bytes.starts_with(blueprint::SLOT_MARKER) && *bytes == empty
                        })
                {
                    let position = segment.range.end - segment.data.len() + index;
                    ensure!(
                        offset.replace(position).is_none(),
                        "guest template contains duplicate blueprint slots"
                    );
                }
            }
        }
    }
    let offset = offset.context("guest template has no empty blueprint slot")?;
    let mut core = template.to_vec();
    let slot = blueprint::embedded_slot(payload).map_err(anyhow::Error::msg)?;
    core[offset..offset + slot.len()].copy_from_slice(&slot);
    Ok(core)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_encoder::{ConstExpr, DataSection, Module};

    #[test]
    fn fills_only_the_active_wasm_data_slot() -> Result<()> {
        let empty = blueprint::embedded_slot(&[]).unwrap();
        let payload = encode_blueprint(&demo())?;
        let mut data = DataSection::new();
        data.active(0, &ConstExpr::i32_const(0), empty.clone());
        let mut template = Module::new();
        template.section(&CustomSection {
            name: Cow::Borrowed("decoy"),
            data: Cow::Borrowed(&empty),
        });
        template.section(&data);
        let patched = populate_template(&template.finish(), &payload)?;
        for section in wasmparser::Parser::new(0).parse_all(&patched) {
            match section? {
                wasmparser::Payload::CustomSection(section) if section.name() == "decoy" => {
                    assert!(
                        blueprint::embedded_payload(section.data())
                            .unwrap()
                            .is_empty()
                    );
                }
                wasmparser::Payload::DataSection(section) => {
                    let segment = section.into_iter().next().unwrap()?;
                    assert_eq!(
                        blueprint::embedded_payload(segment.data),
                        Some(payload.as_slice())
                    );
                }
                _ => {}
            }
        }
        Ok(())
    }
}

fn component(core: &[u8]) -> Result<Vec<u8>> {
    let mut component = wit_component::ComponentEncoder::default()
        .module(core)?
        .validate(true)
        .encode()?;
    let section = CustomSection {
        name: Cow::Borrowed("cinnabar:grants"),
        data: Cow::Borrowed(include_bytes!("../../guest/grants.json")),
    };
    component.push(0);
    section.encode(&mut component);
    Ok(component)
}

fn write_blueprint(schematic: &Schematic, output: &Path) -> Result<()> {
    let bytes = encode_blueprint(schematic)?;
    fs::write(output, bytes)?;
    println!(
        "{}: {:?}, {} blocks, {} palette entries",
        schematic.name,
        schematic.size(),
        schematic.block_count(),
        schematic.palette().len()
    );
    Ok(())
}

fn encode_blueprint(schematic: &Schematic) -> Result<Vec<u8>> {
    ensure!(
        schematic.block_count() > 0,
        "schematic contains no non-air blocks"
    );
    blueprint::encode(schematic).map_err(anyhow::Error::msg)
}

fn demo() -> Schematic {
    let mut out = Schematic::new("Demo arch", [5, 4, 2]);
    let stone = BlockState::new("minecraft:stone");
    let glass = BlockState::new("minecraft:glass");
    for x in 0..5 {
        for y in 0..4 {
            if x == 0 || x == 4 || y == 3 {
                out.set([x, y, 0], stone.clone());
            }
        }
    }
    out.set([2, 3, 1], glass);
    out
}
