#[path = "build/locales.rs"]
mod locales;

use std::{collections::BTreeSet, fs, io, path::Path};

fn collect_template_artifacts(
    root: &Path,
    directory: &Path,
    paths: &mut BTreeSet<String>,
) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let kind = entry.file_type()?;
        if kind.is_dir() {
            collect_template_artifacts(root, &path, paths)?;
        } else if kind.is_file()
            && matches!(
                path.extension().and_then(|ext| ext.to_str()),
                Some("list" | "yaml" | "srs")
            )
            && entry.metadata()?.len() != 0
        {
            let relative = path.strip_prefix(root).map_err(io::Error::other)?;
            let relative = relative
                .to_str()
                .ok_or_else(|| io::Error::other("Non-UTF-8 template artifact path"))?;
            if relative.contains(['\n', '\r']) {
                return Err(io::Error::other("Invalid template artifact path"));
            }
            paths.insert(relative.replace(std::path::MAIN_SEPARATOR, "/"));
        }
    }
    Ok(())
}

fn write_template_artifacts(output: &Path) -> io::Result<()> {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .ok_or_else(|| io::Error::other("Cargo did not provide the manifest directory"))?;
    let root = Path::new(&manifest)
        .ancestors()
        .nth(2)
        .ok_or_else(|| io::Error::other("Missing workspace root"))?;
    let mut paths = BTreeSet::new();
    for relative in [
        "rulesets/RULE-SET",
        "rulesets/Mihomo",
        "rulesets/SingBox",
        "rulesets/AdBlock",
    ] {
        let directory = root.join(relative);
        println!("cargo:rerun-if-changed={}", directory.display());
        collect_template_artifacts(root, &directory, &mut paths)?;
    }
    fs::write(
        output.join("template-artifacts.txt"),
        paths.into_iter().collect::<Vec<_>>().join("\n"),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=build/locales.rs");
    let mut sources = Vec::new();
    for locale in locales::LOCALES {
        let path = format!("assets/locales/{locale}.json");
        println!("cargo:rerun-if-changed={path}");
        sources.push(std::fs::read_to_string(path)?);
    }
    let generated = locales::compile_catalogs([&sources[0], &sources[1], &sources[2]])
        .map_err(std::io::Error::other)?;
    let output = std::env::var_os("OUT_DIR")
        .ok_or_else(|| std::io::Error::other("Cargo did not provide the build output directory"))?;
    let output = std::path::PathBuf::from(output);
    fs::write(output.join("locales.rs"), generated)?;
    if std::env::var_os("CARGO_FEATURE_GUI").is_some() {
        write_template_artifacts(&output)?;
    }
    Ok(())
}
