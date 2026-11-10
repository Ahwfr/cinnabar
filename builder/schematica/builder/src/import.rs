use anyhow::{Context, Result, anyhow, bail, ensure};
use nucleation::{
    blockpedia,
    formats::{classic_schematic, limits::DecodeLimits, litematic, schematic},
    nbt::{
        Endian, NbtValue,
        io::{NbtReadLimits, read_nbt_with_limits},
    },
};
use schematica_core::{BlockState, Schematic, StateValue, blueprint};
use std::{collections::HashMap, fs, io::Read, path::Path};

const FILE_LIMIT: usize = 8 * 1024 * 1024;

pub fn import(path: &Path) -> Result<Schematic> {
    let mut data = Vec::new();
    fs::File::open(path)?
        .take(FILE_LIMIT as u64 + 1)
        .read_to_end(&mut data)?;
    ensure!(data.len() <= FILE_LIMIT, "input exceeds 8 MiB");
    let name = path.file_stem().unwrap_or_default().to_string_lossy();
    let name: String = name
        .chars()
        .filter(|c| !c.is_control() && *c != '§')
        .take(32)
        .collect();
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    import_bytes(&data, name, &ext)
}

fn import_bytes(data: &[u8], name: String, ext: &str) -> Result<Schematic> {
    if ext == "mcstructure" {
        return import_bedrock(data, name);
    }
    let limits = DecodeLimits {
        max_input_bytes: FILE_LIMIT,
        max_decompressed_bytes: FILE_LIMIT,
        max_dimension: blueprint::MAX_EDGE as usize,
        max_volume: blueprint::MAX_CELLS,
        max_regions: 64,
        max_palette_entries: 8192,
        max_entities: 1024,
        max_block_entities: blueprint::MAX_CELLS,
        max_nbt_depth: 32,
        max_nbt_string_bytes: 32 * 1024,
        max_nbt_collection_items: 65_536,
        max_nbt_nodes: 262_144,
    };
    let java = match ext {
        "litematic" => litematic::from_litematic_bounded(data, &limits)?,
        "schem" => schematic::from_schematic_bounded(data, &limits)?,
        "schematic" => classic_schematic::from_classic_schematic_bounded(data, &limits)?,
        _ => bail!("expected .mcstructure, .litematic, .schem or .schematic"),
    };
    let bounds = java
        .get_schematic_bounding_box()
        .context("empty Java schematic")?;
    let min = [bounds.min.0, bounds.min.1, bounds.min.2];
    let max = [bounds.max.0, bounds.max.1, bounds.max.2];
    let [sx, sy, sz] =
        [0, 1, 2].map(|i| i32::try_from(i64::from(max[i]) - i64::from(min[i]) + 1).unwrap_or(0));
    blueprint::validate_size([sx, sy, sz]).map_err(anyhow::Error::msg)?;
    let mut out = Schematic::new(name, [sx, sy, sz]);
    let mut translations = HashMap::new();
    for y in 0..sy {
        for z in 0..sz {
            for x in 0..sx {
                let Some(state) = java.get_block(min[0] + x, min[1] + y, min[2] + z) else {
                    continue;
                };
                if state.get_name() == "minecraft:structure_void" {
                    out.set([x, y, z], BlockState::new(state.get_name()));
                    continue;
                }
                if matches!(
                    state.get_name(),
                    "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
                ) {
                    continue;
                }
                let key = state.to_string();
                if !translations.contains_key(&key) {
                    let translated = blockpedia::BlockState::parse(&key)
                        .and_then(|state| state.to_bedrock())
                        .map_err(|e| anyhow!("cannot translate {key} to Bedrock: {e}"))?;
                    let mut mapped = BlockState::new(translated.id());
                    for (key, value) in translated.properties() {
                        mapped
                            .states
                            .insert(key.to_string(), StateValue::parse(value));
                    }
                    translations.insert(key.clone(), mapped);
                }
                out.set([x, y, z], translations[&key].clone());
            }
        }
    }
    Ok(out)
}

fn import_bedrock(mut bytes: &[u8], name: String) -> Result<Schematic> {
    let root = read_nbt_with_limits(
        &mut bytes,
        Endian::Little,
        NbtReadLimits {
            max_depth: 32,
            max_string_bytes: 32 * 1024,
            max_collection_items: 65_536,
            max_nodes: 262_144,
        },
    )
    .context("read little-endian Bedrock structure NBT")?;
    let root = compound(&root)?;
    ensure!(
        matches!(root.get("format_version"), Some(NbtValue::Int(1))),
        "expected Bedrock structure format version 1"
    );
    let size = list(root.get("size").context("missing size")?)?;
    let [NbtValue::Int(sx), NbtValue::Int(sy), NbtValue::Int(sz)] = size else {
        bail!("expected three integer structure dimensions");
    };
    let [sx, sy, sz] = [*sx, *sy, *sz];
    let volume = blueprint::validate_size([sx, sy, sz]).map_err(anyhow::Error::msg)?;
    let structure = compound(root.get("structure").context("missing structure")?)?;
    let palette = compound(structure.get("palette").context("missing palette")?)?;
    let palette = compound(palette.get("default").context("missing default palette")?)?;
    let palette = list(
        palette
            .get("block_palette")
            .context("missing block palette")?,
    )?;
    let palette: Vec<_> = palette.iter().map(bedrock_state).collect::<Result<_>>()?;
    let layers = list(
        structure
            .get("block_indices")
            .context("missing block indices")?,
    )?;
    let primary = list(layers.first().context("missing primary block layer")?)?;
    ensure!(
        primary.len() == volume,
        "primary block layer length differs from size"
    );
    if let Some(layer) = layers.get(1) {
        let layer = list(layer)?;
        ensure!(
            layer.len() == volume,
            "secondary block layer length differs from size"
        );
        if layer.iter().any(|tag| !matches!(tag, NbtValue::Int(-1))) {
            eprintln!("Warning: the secondary block layer (such as waterlogging) is omitted.");
        }
    }
    let mut out = Schematic::new(name, [sx, sy, sz]);
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                let index = ((x * sy + y) * sz + z) as usize;
                match primary[index] {
                    NbtValue::Int(-1) => {
                        out.set([x, y, z], BlockState::new("minecraft:structure_void"));
                    }
                    NbtValue::Int(i) => {
                        let state = palette
                            .get(i as usize)
                            .with_context(|| format!("invalid palette index {i}"))?;
                        if state.name != "minecraft:air" {
                            out.set([x, y, z], state.clone());
                        }
                    }
                    _ => bail!("expected integer block index at cell {index}"),
                }
            }
        }
    }
    Ok(out)
}

fn bedrock_state(tag: &NbtValue) -> Result<BlockState> {
    let entry = compound(tag)?;
    let Some(NbtValue::String(name)) = entry.get("name") else {
        bail!("block has no name")
    };
    let mut block = BlockState::new(name);
    for (key, tag) in compound(entry.get("states").context("block has no state object")?)?.iter() {
        let value = match tag {
            NbtValue::Byte(0) => StateValue::Bool(false),
            NbtValue::Byte(1) => StateValue::Bool(true),
            NbtValue::Byte(v) => StateValue::Byte(*v),
            NbtValue::Int(v) => StateValue::Int(*v),
            NbtValue::String(v) => StateValue::Str(v.clone()),
            _ => bail!("unsupported NBT state type for {name}.{key}"),
        };
        block.states.insert(key.clone(), value);
    }
    Ok(block)
}

fn compound(tag: &NbtValue) -> Result<&nucleation::nbt::NbtMap> {
    match tag {
        NbtValue::Compound(v) => Ok(v),
        _ => bail!("expected an NBT compound"),
    }
}

fn list(tag: &NbtValue) -> Result<&[NbtValue]> {
    match tag {
        NbtValue::List(v) => Ok(v),
        _ => bail!("expected an NBT list"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nucleation::{UniversalSchematic, formats::mcstructure};

    #[test]
    fn import_preserves_asymmetric_coordinates_and_states() -> Result<()> {
        let mut source = UniversalSchematic::new("fixture".into());
        assert!(source.set_block_str(0, 0, 0, "minecraft:stone"));
        assert!(source.set_block_str(
            1,
            2,
            3,
            "minecraft:oak_stairs[facing=north,half=top,shape=straight,waterlogged=false]"
        ));
        let cases = [
            ("litematic", litematic::to_litematic(&source)?),
            ("schem", schematic::to_schematic(&source)?),
            ("mcstructure", mcstructure::to_mcstructure(&source)?),
        ];
        for (extension, data) in cases {
            let output = import_bytes(&data, extension.to_owned(), extension)?;
            assert_eq!(output.size(), [2, 3, 4], "{extension}");
            assert_eq!(output.block_count(), 2, "{extension}");
            assert_eq!(
                output.get([0, 0, 0]).unwrap().name,
                "minecraft:stone",
                "{extension}"
            );
            let stair = output.get([1, 2, 3]).unwrap();
            assert_eq!(stair.name, "minecraft:oak_stairs", "{extension}");
            assert_eq!(
                stair.states["upside_down_bit"],
                StateValue::Bool(true),
                "{extension}"
            );
            assert_eq!(
                stair.states["weirdo_direction"],
                StateValue::Int(3),
                "{extension}"
            );
        }
        Ok(())
    }
}
