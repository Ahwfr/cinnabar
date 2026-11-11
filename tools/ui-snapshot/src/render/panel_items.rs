//! Uses the installed, hash-validated icon carrier for authored material previews.

use crate::carriers::Error;
use jsonui_editor::raster::Page;
use std::{collections::HashMap, path::Path};
use ui::{IconRef, mod_panel::Panel};

pub fn load(
    panel: &Panel,
    dir: &Path,
    pages: &mut Vec<Page>,
) -> Result<HashMap<(String, u32), IconRef>, Error> {
    let path = dir.join(assets::carriers::ICON.output);
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    let catalog = assets::RuntimeIconCatalog::decode(&bytes).map_err(|error| error.to_string())?;
    let mut icons = HashMap::new();
    for item in panel
        .surface
        .bindings
        .values()
        .filter_map(|value| value.item())
    {
        let key = (item.identifier.clone(), item.metadata);
        if icons.contains_key(&key) {
            continue;
        }
        let Some(sprite) = catalog.lookup(&item.identifier, item.metadata) else {
            continue;
        };
        let page = u16::try_from(pages.len()).map_err(|error| error.to_string())?;
        pages.push(Page {
            repeat_content: None,
            width: u32::from(sprite.width),
            height: u32::from(sprite.height),
            rgba: sprite.rgba8.clone(),
        });
        icons.insert(
            key,
            IconRef {
                color_mask: None,
                page,
                uv: [0, 0, sprite.width, sprite.height],
                glint: false,
            },
        );
    }
    Ok(icons)
}
