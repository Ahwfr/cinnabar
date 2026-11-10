use std::{
    ffi::OsString, mem::size_of, os::windows::ffi::OsStringExt, path::PathBuf, ptr::null_mut,
};
use windows_sys::Win32::UI::{
    Controls::Dialogs::{
        CommDlgExtendedError, GetOpenFileNameW, OFN_ALLOWMULTISELECT, OFN_EXPLORER,
        OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR, OPENFILENAMEW,
    },
    WindowsAndMessaging::{MB_ICONERROR, MB_ICONINFORMATION, MB_OK, MessageBoxW},
};

pub fn run() {
    let filter = wide("Minecraft schematics\0*.schematic;*.schem;*.litematic;*.mcstructure\0");
    let title = wide("Select up to 8 schematics for one library");
    let mut path = vec![0u16; 32768];
    let mut picker = OPENFILENAMEW {
        lStructSize: size_of::<OPENFILENAMEW>() as u32,
        lpstrFilter: filter.as_ptr(),
        lpstrFile: path.as_mut_ptr(),
        nMaxFile: path.len() as u32,
        lpstrTitle: title.as_ptr(),
        Flags: OFN_ALLOWMULTISELECT | OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_NOCHANGEDIR,
        ..Default::default()
    };
    // The caller owns these buffers until the modal dialog returns.
    if unsafe { GetOpenFileNameW(&mut picker) } == 0 {
        let error = unsafe { CommDlgExtendedError() };
        if error != 0 {
            message(
                &format!("The file picker failed (Windows error {error})."),
                MB_ICONERROR,
            );
        }
        return;
    }
    let mut parts = path
        .split(|&ch| ch == 0)
        .take_while(|part| !part.is_empty())
        .map(|part| PathBuf::from(OsString::from_wide(part)));
    let Some(first) = parts.next() else { return };
    let mut inputs: Vec<_> = parts.map(|name| first.join(name)).collect();
    if inputs.is_empty() {
        inputs.push(first);
    }
    match super::convert_selection(&inputs) {
        Ok(output) => message(
            &format!(
                "Created {}\n\nImport this file in Cinnabar > Settings > Mods.",
                output.display()
            ),
            MB_ICONINFORMATION,
        ),
        Err(error) => message(&format!("{error:#}"), MB_ICONERROR),
    }
}

fn message(body: &str, icon: u32) {
    let body = wide(body);
    let title = wide("Schematica Import");
    unsafe { MessageBoxW(null_mut(), body.as_ptr(), title.as_ptr(), MB_OK | icon) };
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0]).collect()
}
