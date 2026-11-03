use editpe::{DataDirectoryType, Image, ResourceDirectory, ResourceEntryName};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

const MAX_PE: u64 = 256 * 1024 * 1024;
const MAX_ICO: usize = 16 * 1024 * 1024;
const SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];

fn read_pe(path: &Path) -> Result<Vec<u8>, String> {
    let meta = fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !meta.is_file() || meta.len() == 0 || meta.len() > MAX_PE {
        return Err(format!("Invalid staged executable: {}", path.display()));
    }
    let data = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if data.len() as u64 > MAX_PE {
        return Err("Executable exceeds size limit".into());
    }
    Ok(data)
}

fn u32le(data: &[u8]) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        data.try_into().map_err(|_| "Truncated integer")?,
    ))
}

fn icons(data: &[u8]) -> Result<Vec<&[u8]>, String> {
    if data.len() > MAX_ICO || data.get(..6) != Some(&[0, 0, 1, 0, 7, 0]) {
        return Err("Invalid ICO header or size".into());
    }
    let mut result = Vec::new();
    let mut next = 6 + 16 * SIZES.len();
    for (i, size) in SIZES.iter().copied().enumerate() {
        let entry = data
            .get(6 + i * 16..6 + (i + 1) * 16)
            .ok_or("Truncated ICO entry")?;
        let dimension = if size == 256 { 0 } else { size as u8 };
        if entry[..8] != [dimension, dimension, 0, 0, 1, 0, 32, 0] {
            return Err("Invalid ICO entry".into());
        }
        let len = u32le(&entry[8..12])? as usize;
        let offset = u32le(&entry[12..16])? as usize;
        if offset != next || len < 24 {
            return Err("Invalid ICO payload bounds".into());
        }
        next = offset.checked_add(len).ok_or("ICO offset overflow")?;
        let png = data.get(offset..next).ok_or("Truncated ICO payload")?;
        if png.get(..8) != Some(b"\x89PNG\r\n\x1a\n")
            || png.get(12..16) != Some(b"IHDR")
            || u32::from_be_bytes(png[16..20].try_into().map_err(|_| "PNG width")?) != size
            || u32::from_be_bytes(png[20..24].try_into().map_err(|_| "PNG height")?) != size
        {
            return Err("Invalid ICO PNG dimensions".into());
        }
        result.push(png);
    }
    if next != data.len() {
        return Err("Unexpected trailing ICO data".into());
    }
    Ok(result)
}

fn unsigned(image: &Image<'_>) -> Result<(), String> {
    if let Some(directory) = image.data_directory(DataDirectoryType::CertificateTable)
        && (directory.virtual_address != 0 || directory.size != 0)
    {
        return Err("Refusing to modify a signed PE executable".into());
    }
    Ok(())
}

fn group(directory: &ResourceDirectory, ico: &[u8]) -> Result<(), String> {
    let expected = icons(ico)?;
    let root = directory.root();
    let groups = root
        .get(ResourceEntryName::ID(14))
        .and_then(|e| e.as_table())
        .ok_or("Missing icon groups")?;
    let main = groups
        .get(ResourceEntryName::from_string("MAINICON"))
        .and_then(|e| e.as_table())
        .ok_or("Missing MAINICON")?;
    let data = main
        .get(ResourceEntryName::ID(1033))
        .and_then(|e| e.as_data())
        .ok_or("Missing icon group language")?
        .data();
    if data.len() != 6 + 14 * SIZES.len() || data[..6] != [0, 0, 1, 0, 7, 0] {
        return Err("Invalid icon group".into());
    }
    let table = root
        .get(ResourceEntryName::ID(3))
        .and_then(|e| e.as_table())
        .ok_or("Missing icon resources")?;
    for (i, payload) in expected.iter().enumerate() {
        let entry = &data[6 + i * 14..6 + (i + 1) * 14];
        let src = &ico[6 + i * 16..6 + i * 16 + 12];
        if entry[..12] != *src {
            return Err("Icon group metadata differs from source ICO".into());
        }
        let id = u16::from_le_bytes([entry[12], entry[13]]);
        let image = table
            .get(ResourceEntryName::ID(u32::from(id)))
            .and_then(|e| e.as_table())
            .and_then(|t| t.get(ResourceEntryName::ID(1033)))
            .and_then(|e| e.as_data())
            .ok_or("Missing icon image")?
            .data();
        if image != *payload {
            return Err("Icon image differs from source ICO".into());
        }
    }
    Ok(())
}

fn parse(data: &[u8]) -> Result<Image<'_>, String> {
    let image = Image::parse(data).map_err(|e| format!("Invalid PE: {e}"))?;
    unsigned(&image)?;
    Ok(image)
}

pub(crate) fn verify(path: &Path, ico: &[u8]) -> Result<(), String> {
    let bytes = read_pe(path)?;
    let image = parse(&bytes)?;
    group(
        image.resource_directory().ok_or("Missing PE resources")?,
        ico,
    )
}

pub(crate) fn apply(path: &Path, ico: &[u8]) -> Result<(), String> {
    icons(ico)?;
    let before = read_pe(path)?;
    let original = parse(&before)?;
    let mut image = original.cloned();
    let mut resources = original.resource_directory().cloned().unwrap_or_default();
    resources
        .set_main_icon(ico)
        .map_err(|e| format!("Cannot set PE icon: {e}"))?;
    image
        .set_resource_directory(resources)
        .map_err(|e| format!("Cannot write PE resources: {e}"))?;
    let output = image.data();
    let checked = parse(output)?;
    if checked.subsystem() != original.subsystem()
        || checked.standard_header().address_of_entry_point
            != original.standard_header().address_of_entry_point
        || checked.coff_header().machine != original.coff_header().machine
        || checked.coff_header().characteristics != original.coff_header().characteristics
    {
        return Err("Essential PE headers changed".into());
    }
    let mut old_resources = original.resource_directory().cloned().unwrap_or_default();
    let mut new_resources = checked
        .resource_directory()
        .cloned()
        .ok_or("Missing rewritten resources")?;
    group(&new_resources, ico)?;
    for kind in [3, 14] {
        old_resources.root_mut().remove(ResourceEntryName::ID(kind));
        new_resources.root_mut().remove(ResourceEntryName::ID(kind));
    }
    let old_root = old_resources.root();
    let new_root = new_resources.root();
    if old_root.entries().len() != new_root.entries().len()
        || old_root
            .entries()
            .iter()
            .any(|name| old_root.get(*name) != new_root.get(*name))
    {
        return Err("Unrelated PE resources changed".into());
    }
    let temp = path.with_extension("exe.icon-new");
    let backup = path.with_extension("exe.icon-original");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|e| format!("{}: {e}", temp.display()))?;
    file.write_all(output)
        .and_then(|()| file.sync_all())
        .map_err(|e| format!("{}: {e}", temp.display()))?;
    drop(file);
    verify(&temp, ico)?;
    // The source is an isolated staged copy, not the compiler output.
    if fs::symlink_metadata(&backup).is_ok() {
        return Err(format!("Backup already exists: {}", backup.display()));
    }
    fs::rename(path, &backup).map_err(|e| format!("Cannot preserve staged executable: {e}"))?;
    if let Err(error) = fs::rename(&temp, path) {
        return match fs::rename(&backup, path) {
            Ok(()) => Err(format!("Cannot publish staged icon: {error}")),
            Err(rollback) => Err(format!(
                "Cannot publish staged icon: {error}; restore {} manually: {rollback}",
                backup.display()
            )),
        };
    }
    fs::remove_file(&backup)
        .map_err(|e| format!("Cannot remove staged backup {}: {e}", backup.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put16(buf: &mut [u8], offset: usize, value: u16) {
        buf[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }
    fn put32(buf: &mut [u8], offset: usize, value: u32) {
        buf[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    // A synthetic PE32+ header and inert section, never executable test data.
    fn pe() -> Vec<u8> {
        let mut data = vec![0; 1024];
        data[..2].copy_from_slice(b"MZ");
        put32(&mut data, 0x3c, 0x80);
        data[0x80..0x84].copy_from_slice(b"PE\0\0");
        put16(&mut data, 0x84, 0x8664);
        put16(&mut data, 0x86, 1);
        put16(&mut data, 0x94, 240);
        put16(&mut data, 0x96, 0x22);
        let o = 0x98;
        put16(&mut data, o, 0x20b);
        put32(&mut data, o + 16, 0x1000);
        put32(&mut data, o + 20, 0x1000);
        put32(&mut data, o + 32, 0x1000);
        put32(&mut data, o + 36, 0x200);
        put32(&mut data, o + 56, 0x2000);
        put32(&mut data, o + 60, 0x200);
        put16(&mut data, o + 68, 2);
        put32(&mut data, o + 108, 16);
        let s = o + 240;
        data[s..s + 5].copy_from_slice(b".text");
        put32(&mut data, s + 8, 0x100);
        put32(&mut data, s + 12, 0x1000);
        put32(&mut data, s + 16, 0x200);
        put32(&mut data, s + 20, 0x200);
        put32(&mut data, s + 36, 0x60000020);
        data
    }

    fn ico() -> Vec<u8> {
        let mut output = vec![0, 0, 1, 0, 7, 0];
        let mut payloads = Vec::new();
        let mut offset = 6 + 7 * 16;
        for size in SIZES {
            let dim = if size == 256 { 0 } else { size as u8 };
            output.extend_from_slice(&[dim, dim, 0, 0, 1, 0, 32, 0]);
            output.extend_from_slice(&24_u32.to_le_bytes());
            output.extend_from_slice(&(offset as u32).to_le_bytes());
            let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
            png.extend_from_slice(&size.to_be_bytes());
            png.extend_from_slice(&size.to_be_bytes());
            payloads.extend(png);
            offset += 24;
        }
        output.extend(payloads);
        output
    }

    #[test]
    fn synthetic_pe_roundtrip_keeps_manifest_and_all_icon_entries() {
        let bytes = pe();
        let mut image = Image::parse(bytes.as_slice()).unwrap().cloned();
        let mut resources = ResourceDirectory::default();
        resources
            .set_manifest("<assembly>fixture</assembly>")
            .unwrap();
        image.set_resource_directory(resources).unwrap();
        let original = image.resource_directory().unwrap().get_manifest().unwrap();
        let mut resources = image.resource_directory().unwrap().clone();
        resources.set_main_icon(ico().as_slice()).unwrap();
        image.set_resource_directory(resources).unwrap();
        let result = Image::parse(image.data()).unwrap();
        assert_eq!(
            result.resource_directory().unwrap().get_manifest().unwrap(),
            original
        );
        group(result.resource_directory().unwrap(), &ico()).unwrap();
        assert_eq!(result.subsystem(), 2);
    }

    #[test]
    fn invalid_and_signed_pe_are_rejected() {
        assert!(parse(b"not a PE").is_err());
        let mut signed = pe();
        put32(&mut signed, 0x98 + 112 + 4 * 8, 0x300);
        put32(&mut signed, 0x98 + 112 + 4 * 8 + 4, 8);
        assert!(parse(&signed).is_err());
        assert!(icons(&[]).is_err());
        let mut bad = ico();
        put32(&mut bad, 18, u32::MAX);
        assert!(icons(&bad).is_err());
    }

    #[test]
    fn staged_icon_publication_preserves_input_and_manifest() {
        let directory = std::env::temp_dir().join(format!("script-hub-icon-{}", crate::nonce()));
        fs::create_dir(&directory).unwrap();
        let source = directory.join("source.exe");
        let staged = directory.join("staged.exe");
        let bytes = pe();
        let mut original = Image::parse(bytes.as_slice()).unwrap().cloned();
        let mut resources = ResourceDirectory::default();
        resources
            .set_manifest("<assembly>preserve</assembly>")
            .unwrap();
        original.set_resource_directory(resources).unwrap();
        fs::write(&source, original.data()).unwrap();
        fs::copy(&source, &staged).unwrap();
        let before = fs::read(&staged).unwrap();
        assert!(apply(&staged, b"invalid").is_err());
        assert_eq!(fs::read(&staged).unwrap(), before);
        let icon = include_bytes!("../../processor/assets/ScriptHub.ico");
        apply(&staged, icon).unwrap();
        verify(&staged, icon).unwrap();
        assert_eq!(fs::read(&source).unwrap(), before);
        let updated = fs::read(&staged).unwrap();
        let image = Image::parse(updated.as_slice()).unwrap();
        assert_eq!(
            image.resource_directory().unwrap().get_manifest().unwrap(),
            Some("<assembly>preserve</assembly>".into())
        );
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 2);
        fs::remove_file(staged).unwrap();
        fs::remove_file(source).unwrap();
        fs::remove_dir(directory).unwrap();
    }
}
