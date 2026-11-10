use crate::{BlockState, Schematic, StateValue};

pub const MAX_EDGE: i32 = 64;
pub const MAX_CELLS: usize = 5120;
const MAX_PALETTE: usize = 128;
pub const SLOT_MARKER: &[u8] = b"schematica:embedded-library:v4\x00";
const PAYLOAD_BYTES: usize = MAX_CELLS + 12 * 1024;
pub const MAX_LIBRARY: usize = 8;
const LIBRARY_BYTES: usize = 5 + MAX_LIBRARY * (4 + PAYLOAD_BYTES);
pub const SLOT_BYTES: usize = SLOT_MARKER.len() + 4 + LIBRARY_BYTES;

pub const fn empty_slot() -> [u8; SLOT_BYTES] {
    let mut slot = [0; SLOT_BYTES];
    let mut index = 0;
    while index < SLOT_MARKER.len() {
        slot[index] = SLOT_MARKER[index];
        index += 1;
    }
    slot
}

/// Encodes independent blueprints so the guest can decode only the selected one.
pub fn encode_library(blueprints: &[Vec<u8>]) -> Result<Vec<u8>, String> {
    if !(1..=MAX_LIBRARY).contains(&blueprints.len()) {
        return Err(format!("choose between 1 and {MAX_LIBRARY} schematics"));
    }
    let mut bytes = b"SCL1".to_vec();
    bytes.push(blueprints.len() as u8);
    for payload in blueprints {
        if payload.len() > PAYLOAD_BYTES {
            return Err("blueprint exceeds the library entry limit".into());
        }
        bytes.extend((payload.len() as u32).to_le_bytes());
        bytes.extend(payload);
    }
    Ok(bytes)
}

pub fn library(mut bytes: &[u8]) -> Result<Vec<&[u8]>, String> {
    if bytes.starts_with(b"SCB1") {
        return Ok(vec![bytes]);
    }
    if take(&mut bytes, 4)? != b"SCL1" {
        return Err("invalid blueprint library".into());
    }
    let count = byte(&mut bytes)? as usize;
    if !(1..=MAX_LIBRARY).contains(&count) {
        return Err("invalid blueprint count".into());
    }
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let len = u32::from_le_bytes(take(&mut bytes, 4)?.try_into().unwrap()) as usize;
        if len > PAYLOAD_BYTES {
            return Err("blueprint exceeds the library entry limit".into());
        }
        entries.push(take(&mut bytes, len)?);
    }
    if !bytes.is_empty() {
        return Err("unexpected data after blueprint library".into());
    }
    Ok(entries)
}

pub fn title(bytes: &[u8]) -> Result<String, String> {
    Ok(Decoder::new(bytes)?.name)
}

/// Builds a fixed-size data slot that the desktop converter can populate without recompiling.
pub fn embedded_slot(payload: &[u8]) -> Result<Vec<u8>, String> {
    if payload.len() > LIBRARY_BYTES {
        return Err("blueprint exceeds the embedded slot".into());
    }
    let mut slot = SLOT_MARKER.to_vec();
    slot.extend((payload.len() as u32).to_le_bytes());
    slot.extend_from_slice(payload);
    slot.resize(SLOT_BYTES, 0);
    Ok(slot)
}

pub fn embedded_payload(slot: &[u8]) -> Option<&[u8]> {
    let header = slot.strip_prefix(SLOT_MARKER)?;
    let (len, bytes) = header.split_at_checked(4)?;
    bytes.get(..u32::from_le_bytes(len.try_into().ok()?) as usize)
}

pub fn validate_size(size: [i32; 3]) -> Result<usize, String> {
    if !size.iter().all(|n| (1..=MAX_EDGE).contains(n)) {
        return Err(format!("each dimension must be 1–{MAX_EDGE}, got {size:?}"));
    }
    let cells = size.iter().product::<i32>() as usize;
    if cells > MAX_CELLS {
        return Err(format!(
            "bounding box has {cells} cells, limit is {MAX_CELLS}"
        ));
    }
    Ok(cells)
}

pub fn encode(schematic: &Schematic) -> Result<Vec<u8>, String> {
    let [sx, sy, sz] = schematic.size();
    validate_size([sx, sy, sz])?;
    if schematic.palette().len() > MAX_PALETTE {
        return Err(format!(
            "palette exceeds {MAX_PALETTE} distinct block states"
        ));
    }
    let mut out = b"SCB1".to_vec();
    out.extend([
        sx as u8,
        sy as u8,
        sz as u8,
        schematic.palette().len() as u8,
    ]);
    put_str(&mut out, &schematic.name)?;
    for block in schematic.palette() {
        put_str(&mut out, &block.name)?;
        out.push(u8::try_from(block.states.len()).map_err(|_| "too many block properties")?);
        for (name, value) in &block.states {
            put_str(&mut out, name)?;
            match value {
                StateValue::Bool(v) => out.extend([0, u8::from(*v)]),
                StateValue::Int(v) => {
                    out.push(1);
                    out.extend(v.to_le_bytes());
                }
                StateValue::Str(v) => {
                    out.push(2);
                    put_str(&mut out, v)?;
                }
                StateValue::Byte(v) => out.extend([3, *v as u8]),
            }
        }
    }
    if out.len() > 12 * 1024 {
        return Err("encoded palette exceeds 12 KiB".into());
    }
    for y in 0..sy {
        for z in 0..sz {
            for x in 0..sx {
                out.push(schematic.palette_index([x, y, z]).unwrap() as u8);
            }
        }
    }
    Ok(out)
}

fn put_str(out: &mut Vec<u8>, value: &str) -> Result<(), String> {
    out.push(u8::try_from(value.len()).map_err(|_| "string exceeds 255 bytes")?);
    out.extend(value.as_bytes());
    Ok(())
}

pub fn decode(bytes: &[u8]) -> Result<Schematic, String> {
    let mut decoder = Decoder::new(bytes)?;
    while !decoder.step()? {}
    decoder.finish()
}

/// Incrementally decodes one palette entry at a time before copying the bounded cell array.
pub struct Decoder<'a> {
    bytes: &'a [u8],
    size: [i32; 3],
    count: usize,
    name: String,
    palette: Vec<BlockState>,
}

impl<'a> Decoder<'a> {
    pub fn new(mut bytes: &'a [u8]) -> Result<Self, String> {
        if take(&mut bytes, 4)? != b"SCB1" {
            return Err("expected a SCB1 blueprint".into());
        }
        let size = [
            byte(&mut bytes)? as i32,
            byte(&mut bytes)? as i32,
            byte(&mut bytes)? as i32,
        ];
        let volume = validate_size(size)?;
        let count = byte(&mut bytes)? as usize;
        if !(1..=MAX_PALETTE).contains(&count) || bytes.len() > volume + 12 * 1024 {
            return Err("invalid palette size".into());
        }
        let name = string(&mut bytes)?;
        Ok(Self {
            bytes,
            size,
            count,
            name,
            palette: Vec::with_capacity(count),
        })
    }

    pub fn step(&mut self) -> Result<bool, String> {
        if self.palette.len() == self.count {
            return Ok(true);
        }
        let bytes = &mut self.bytes;
        let mut state = BlockState::new(string(bytes)?);
        for _ in 0..byte(bytes)? {
            let key = string(bytes)?;
            let value = match byte(bytes)? {
                0 => StateValue::Bool(byte(bytes)? != 0),
                1 => StateValue::Int(i32::from_le_bytes(take(bytes, 4)?.try_into().unwrap())),
                2 => StateValue::Str(string(bytes)?),
                3 => StateValue::Byte(byte(bytes)? as i8),
                _ => return Err("unknown state value type".into()),
            };
            state.states.insert(key, value);
        }
        self.palette.push(state);
        Ok(self.palette.len() == self.count)
    }

    pub fn finish(self) -> Result<Schematic, String> {
        if self.palette.len() != self.count {
            return Err("blueprint palette has not finished decoding".into());
        }
        Schematic::from_raw(
            self.name,
            self.size,
            self.palette,
            self.bytes.iter().map(|&b| u32::from(b)).collect(),
        )
    }
}

fn take<'a>(bytes: &mut &'a [u8], size: usize) -> Result<&'a [u8], String> {
    let (prefix, rest) = bytes.split_at_checked(size).ok_or("truncated blueprint")?;
    *bytes = rest;
    Ok(prefix)
}

fn byte(bytes: &mut &[u8]) -> Result<u8, String> {
    Ok(take(bytes, 1)?[0])
}

fn string(bytes: &mut &[u8]) -> Result<String, String> {
    let len = byte(bytes)? as usize;
    String::from_utf8(take(bytes, len)?.to_vec()).map_err(|_| "invalid UTF-8".into())
}
