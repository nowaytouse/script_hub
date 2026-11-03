use resvg::{tiny_skia, usvg};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const ICNS: [(&[u8; 4], u32); 11] = [
    (b"icp4", 16),
    (b"icp5", 32),
    (b"icp6", 64),
    (b"ic07", 128),
    (b"ic08", 256),
    (b"ic09", 512),
    (b"ic10", 1024),
    (b"ic11", 32),
    (b"ic12", 64),
    (b"ic13", 256),
    (b"ic14", 512),
];
const ICO: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];
const OUTPUTS: [&str; 3] = ["system-icon.png", "ScriptHub.icns", "ScriptHub.ico"];

fn render(tree: &usvg::Tree, size: u32) -> Result<tiny_skia::Pixmap> {
    let mut image = tiny_skia::Pixmap::new(size, size).ok_or("Cannot allocate icon")?;
    let dimensions = tree.size();
    if dimensions.width() != 1024.0 || dimensions.height() != 1024.0 {
        return Err("Expected the original 1024 x 1024 SVG".into());
    }
    let scale = size as f32 * 0.8 / 1024.0;
    let inset = size as f32 * 0.1;
    let transform = tiny_skia::Transform::from_row(scale, 0.0, 0.0, scale, inset, inset);
    resvg::render(tree, transform, &mut image.as_mut());
    Ok(image)
}

fn pngs(svg: &[u8]) -> Result<BTreeMap<u32, Vec<u8>>> {
    let tree = usvg::Tree::from_data(svg, &usvg::Options::default())?;
    let mut images = BTreeMap::new();
    for size in [16, 24, 32, 48, 64, 128, 256, 512, 1024] {
        images.insert(size, render(&tree, size)?.encode_png()?);
    }
    Ok(images)
}

fn icns(images: &BTreeMap<u32, Vec<u8>>) -> Result<Vec<u8>> {
    let mut out = b"icns\0\0\0\0".to_vec();
    for (kind, size) in ICNS {
        let image = images.get(&size).ok_or("Missing ICNS image")?;
        let length = u32::try_from(image.len().checked_add(8).ok_or("ICNS record overflow")?)?;
        out.extend_from_slice(kind);
        out.extend_from_slice(&length.to_be_bytes());
        out.extend_from_slice(image);
    }
    let length = u32::try_from(out.len())?;
    out[4..8].copy_from_slice(&length.to_be_bytes());
    Ok(out)
}

fn ico(images: &BTreeMap<u32, Vec<u8>>) -> Result<Vec<u8>> {
    let mut out = vec![0, 0, 1, 0, ICO.len() as u8, 0];
    let mut offset = 6_usize + ICO.len() * 16;
    for size in ICO {
        let image = images.get(&size).ok_or("Missing ICO image")?;
        let dimension = if size == 256 { 0 } else { size as u8 };
        out.extend_from_slice(&[dimension, dimension, 0, 0, 1, 0, 32, 0]);
        out.extend_from_slice(&u32::try_from(image.len())?.to_le_bytes());
        out.extend_from_slice(&u32::try_from(offset)?.to_le_bytes());
        offset = offset.checked_add(image.len()).ok_or("ICO size overflow")?;
    }
    for size in ICO {
        out.extend_from_slice(images.get(&size).ok_or("Missing ICO image")?);
    }
    Ok(out)
}

fn no_links(path: &Path) -> Result<()> {
    let mut current = PathBuf::new();
    for part in path.components() {
        if matches!(part, Component::ParentDir) {
            return Err("Parent path component rejected".into());
        }
        current.push(part.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(format!("Symbolic link rejected: {}", current.display()).into());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn regular_or_missing(path: &Path) -> Result<()> {
    no_links(path)?;
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_file() => Ok(()),
        Ok(_) => Err(format!("Not a regular file: {}", path.display()).into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn run() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let check = match (args.next(), args.next()) {
        (None, None) => false,
        (Some(arg), None) if arg == "--check" => true,
        _ => return Err("Usage: cargo icons [--check]".into()),
    };
    // Resolve the fixed repository path before inspecting each component.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("Missing crates directory")?;
    let dir = root.join("processor/assets");
    no_links(&dir)?;
    let source = dir.join("logo.svg");
    no_links(&source)?;
    if !fs::symlink_metadata(&source)?.is_file() {
        return Err("SVG is not a regular file".into());
    }
    let images = pngs(&fs::read(source)?)?;
    let generated = [
        images.get(&256).ok_or("Missing 256px image")?.clone(),
        icns(&images)?,
        ico(&images)?,
    ];
    for name in OUTPUTS {
        regular_or_missing(&dir.join(name))?;
    }
    if check {
        for (name, expected) in OUTPUTS.iter().zip(&generated) {
            if fs::read(dir.join(name))? != *expected {
                return Err(format!("{name} is out of date").into());
            }
        }
        return Ok(());
    }
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let backup_dir = root
        .parent()
        .ok_or("Missing repository directory")?
        .join("target/icon-backups")
        .join(nonce.to_string());
    no_links(&backup_dir)?;
    fs::create_dir_all(&backup_dir)?;
    let mut staged = Vec::new();
    for (name, bytes) in OUTPUTS.iter().zip(&generated) {
        let path = dir.join(format!(".{name}.{nonce}.new"));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        staged.push(path);
    }
    for ((name, temp), expected) in OUTPUTS.iter().zip(&staged).zip(&generated) {
        let dest = dir.join(name);
        regular_or_missing(&dest)?;
        if fs::read(&dest).is_ok_and(|bytes| bytes == *expected) {
            fs::remove_file(temp)?;
            continue;
        }
        let backup = backup_dir.join(name);
        if dest.exists() {
            // A hard link creates the backup without replacing an existing path.
            fs::hard_link(&dest, &backup)?;
            eprintln!("Previous {name} retained at {}", backup.display());
            fs::remove_file(&dest)?;
        }
        if let Err(error) = fs::rename(temp, &dest) {
            return Err(format!(
                "Cannot publish {name}: {error}; previous file, if any, remains at {}",
                backup.display()
            )
            .into());
        }
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Icon generation failed: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (usvg::Tree, BTreeMap<u32, Vec<u8>>) {
        let svg = include_bytes!("../../../processor/assets/logo.svg");
        (
            usvg::Tree::from_data(svg, &usvg::Options::default()).unwrap(),
            pngs(svg).unwrap(),
        )
    }

    #[test]
    fn rendered_icons_have_centered_transparent_margins_at_each_size() {
        let (tree, _) = fixture();
        for size in [16, 24, 32, 48, 64, 128, 256, 512, 1024] {
            let image = render(&tree, size).unwrap();
            let mut bounds = (size, size, 0, 0);
            for y in 0..size {
                for x in 0..size {
                    if image.pixel(x, y).unwrap().alpha() > 0 {
                        bounds.0 = bounds.0.min(x);
                        bounds.1 = bounds.1.min(y);
                        bounds.2 = bounds.2.max(x);
                        bounds.3 = bounds.3.max(y);
                    }
                }
            }
            assert!(
                bounds.0 > 0 && bounds.1 > 0 && bounds.2 < size - 1 && bounds.3 < size - 1,
                "{size}: {bounds:?}"
            );
            assert!(
                (bounds.0 as i32 - (size - 1 - bounds.2) as i32).abs() <= 1,
                "{size}: {bounds:?}"
            );
            assert!(
                (bounds.1 as i32 - (size - 1 - bounds.3) as i32).abs() <= 1,
                "{size}: {bounds:?}"
            );
            assert!(bounds.2 - bounds.0 >= size * 3 / 4, "{size}: {bounds:?}");
        }
    }

    #[test]
    fn original_circular_accent_remains_visible_in_system_icons() {
        let (tree, _) = fixture();
        for size in [64, 128, 256, 512, 1024] {
            let image = render(&tree, size).unwrap();
            let x = (size as f32 * (0.1 + 0.8 * 860.0 / 1024.0)) as u32;
            let y = (size as f32 * (0.1 + 0.8 * 500.0 / 1024.0)) as u32;
            let pixel = image.pixel(x, y).unwrap();
            assert!(
                pixel.red() > 140 && pixel.green() > 130 && pixel.blue() > 180,
                "Accent missing at {size}px: {pixel:?}"
            );
        }
    }

    fn png_size(data: &[u8]) -> Option<(u32, u32)> {
        let image = tiny_skia::Pixmap::decode_png(data).ok()?;
        Some((image.width(), image.height()))
    }

    fn check_icns(data: &[u8]) -> Option<Vec<([u8; 4], u32)>> {
        if data.len() < 8
            || &data[..4] != b"icns"
            || u32::from_be_bytes(data[4..8].try_into().ok()?) as usize != data.len()
        {
            return None;
        }
        let mut result = Vec::new();
        let mut offset = 8_usize;
        while offset < data.len() {
            let header = data.get(offset..offset.checked_add(8)?)?;
            let length = u32::from_be_bytes(header[4..8].try_into().ok()?) as usize;
            if length <= 8 {
                return None;
            }
            let end = offset.checked_add(length)?;
            let (width, height) = png_size(data.get(offset + 8..end)?)?;
            if width != height {
                return None;
            }
            result.push((header[..4].try_into().ok()?, width));
            offset = end;
        }
        Some(result)
    }

    fn check_ico(data: &[u8]) -> Option<Vec<u32>> {
        if data.get(..4)? != [0, 0, 1, 0] {
            return None;
        }
        let count = u16::from_le_bytes(data.get(4..6)?.try_into().ok()?) as usize;
        if count == 0 {
            return None;
        }
        let mut offset = 6_usize.checked_add(count.checked_mul(16)?)?;
        let mut sizes = Vec::new();
        for index in 0..count {
            let entry = data.get(6 + index * 16..6 + (index + 1) * 16)?;
            let dimension = if entry[0] == 0 {
                256
            } else {
                u32::from(entry[0])
            };
            if entry[1] != entry[0] {
                return None;
            }
            let len = u32::from_le_bytes(entry[8..12].try_into().ok()?) as usize;
            let start = u32::from_le_bytes(entry[12..16].try_into().ok()?) as usize;
            if len == 0 || start != offset {
                return None;
            }
            offset = start.checked_add(len)?;
            if png_size(data.get(start..offset)?)? != (dimension, dimension) {
                return None;
            }
            sizes.push(dimension);
        }
        if offset != data.len() {
            return None;
        }
        Some(sizes)
    }

    #[test]
    fn containers_have_expected_images_and_reject_corruption() {
        let (_, images) = fixture();
        let mac = icns(&images).unwrap();
        assert_eq!(
            check_icns(&mac).unwrap(),
            ICNS.iter()
                .map(|(kind, size)| (**kind, *size))
                .collect::<Vec<_>>()
        );
        assert!(check_icns(&[]).is_none());
        let mut bad = mac.clone();
        bad[12..16].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(check_icns(&bad).is_none());
        let windows = ico(&images).unwrap();
        assert_eq!(check_ico(&windows).unwrap(), ICO);
        assert!(check_ico(&[]).is_none());
        let mut bad = windows.clone();
        bad[14..18].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(check_ico(&bad).is_none());
    }
}
