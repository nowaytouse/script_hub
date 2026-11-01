#[path = "build/locales.rs"]
mod locales;

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
    std::fs::write(
        std::path::PathBuf::from(output).join("locales.rs"),
        generated,
    )?;
    Ok(())
}
