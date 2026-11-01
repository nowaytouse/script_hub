//! Native desktop assembly and recoverable repository-root delivery.

use std::{
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    process::Command,
};

const REPOSITORY: &str = "https://github.com/nowaytouse/script_hub";
const IDENTIFIER: &str = "io.github.nowaytouse.script-hub";
const ROOT_FILES: &[&str] = &[
    "LICENSE",
    "PRIVACY.md",
    "PRIVACY.zh-CN.md",
    "SECURITY.md",
    "SECURITY.zh-CN.md",
    "THIRD_PARTY_NOTICES.md",
    "THIRD_PARTY_NOTICES.zh-CN.md",
];
const FONTS: &[&str] = &[
    "Hack-Regular.txt",
    "OFL.txt",
    "UFL.txt",
    "egui-icons.txt",
    "emoji-icon-font-mit-license.txt",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Platform {
    MacOs,
    Windows,
}

struct Metadata {
    version: String,
    commit: String,
    dirty: bool,
    build_number: String,
}

impl Metadata {
    fn new(version: &str, commit: &str, dirty: bool, build_number: &str) -> Result<Self, String> {
        let numeric =
            |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
        if version.split('.').count() != 3
            || !version.split('.').all(numeric)
            || !matches!(commit.len(), 40 | 64)
            || !commit.bytes().all(|byte| byte.is_ascii_hexdigit())
            || !numeric(build_number)
            || build_number.len() > 20
        {
            return Err("Invalid package version, source commit or build number.".to_owned());
        }
        Ok(Self {
            version: version.to_owned(),
            commit: commit.to_owned(),
            dirty,
            build_number: build_number.to_owned(),
        })
    }

    fn source(&self) -> String {
        format!(
            "Version: {}\nSource commit: {}\nSource dirty: {}\nSource: {REPOSITORY}/tree/{}\n",
            self.version, self.commit, self.dirty, self.commit
        )
    }
}

struct Delivery {
    destination: PathBuf,
    backup: Option<PathBuf>,
}

fn context<T>(result: std::io::Result<T>, path: &Path) -> Result<T, String> {
    result.map_err(|error| format!("{}: {error}", path.display()))
}

fn no_links(root: &Path, path: &Path) -> Result<(), String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| "Package path escapes the repository.".to_owned())?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err("Package path contains a non-local component.".to_owned());
        }
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!(
                    "Symbolic links are not package inputs or destinations: {}",
                    current.display()
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("Cannot inspect {}: {error}", current.display())),
        }
    }
    Ok(())
}

fn regular(root: &Path, path: &Path) -> Result<(), String> {
    no_links(root, path)?;
    let metadata = context(fs::symlink_metadata(path), path)?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(format!(
            "Required package input is not a nonempty regular file: {}",
            path.display()
        ));
    }
    Ok(())
}

fn build_tree(root: &Path, directory: &Path) -> Result<(), String> {
    no_links(root, directory)?;
    if directory
        .strip_prefix(root.join("target/release/bundle"))
        .is_err()
        || directory == root.join("target/release/bundle")
    {
        return Err("Staging must use a child of target/release/bundle.".to_owned());
    }
    Ok(())
}

fn create_directory(root: &Path, directory: &Path) -> Result<(), String> {
    no_links(root, directory)?;
    context(fs::create_dir_all(directory), directory)
}

fn copy_checked(root: &Path, source: &Path, destination: &Path) -> Result<(), String> {
    regular(root, source)?;
    create_directory(
        root,
        destination
            .parent()
            .ok_or("Package destination has no parent")?,
    )?;
    context(fs::copy(source, destination), destination)?;
    let mut original = context(fs::File::open(source), source)?;
    let mut copied = context(fs::File::open(destination), destination)?;
    let mut left = [0; 8192];
    let mut right = [0; 8192];
    loop {
        let count = context(original.read(&mut left), source)?;
        if count == 0 {
            if context(copied.read(&mut right[..1]), destination)? != 0 {
                return Err("Package copy grew unexpectedly.".to_owned());
            }
            return Ok(());
        }
        context(copied.read_exact(&mut right[..count]), destination)?;
        if left[..count] != right[..count] {
            return Err(format!(
                "Package copy differs from its source: {}",
                destination.display()
            ));
        }
    }
}

fn resources(platform: Platform) -> &'static str {
    match platform {
        Platform::MacOs => "ScriptHub.app/Contents/Resources",
        Platform::Windows => "ScriptHub.resources",
    }
}

fn entries(platform: Platform) -> &'static [&'static str] {
    match platform {
        Platform::MacOs => &["ScriptHub.app"],
        Platform::Windows => &["ScriptHub.resources", "script-hub-gui.exe"],
    }
}

fn binary(platform: Platform) -> &'static str {
    match platform {
        Platform::MacOs => "script-hub-gui",
        Platform::Windows => "script-hub-gui.exe",
    }
}

fn native_host(verbose: &str, platform: Platform) -> Result<String, String> {
    let mut hosts = verbose
        .lines()
        .filter_map(|line| line.strip_prefix("host: "));
    let host = hosts
        .next()
        .ok_or("Compiler did not report its native host.")?;
    let native = match platform {
        Platform::MacOs => host.ends_with("-apple-darwin"),
        Platform::Windows => {
            host.contains("-pc-windows-")
                && ["-msvc", "-gnu", "-gnullvm"]
                    .iter()
                    .any(|suffix| host.ends_with(suffix))
        }
    };
    if hosts.next().is_some()
        || host.len() > 128
        || !native
        || !host
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err("Compiler host is invalid or is not native to this platform.".to_owned());
    }
    Ok(host.to_owned())
}

fn build_native_with(
    root: &Path,
    platform: Platform,
    host: &str,
    invoke: impl FnOnce(&mut Command) -> Result<(), String>,
) -> Result<PathBuf, String> {
    native_host(&format!("host: {host}"), platform)?;
    let executable = root
        .join("target")
        .join(host)
        .join("release")
        .join(binary(platform));
    for path in [
        root.join("target/release/bundle"),
        root.join("target/app-backups"),
        executable.clone(),
    ] {
        no_links(root, &path)?;
    }
    invoke(
        Command::new("cargo")
            .current_dir(root)
            .args([
                "build",
                "--locked",
                "--release",
                "--package",
                "rust_processor",
                "--features",
                "gui",
                "--bin",
                "script-hub-gui",
                "--jobs",
                "1",
                "--target",
                host,
            ])
            .env("CARGO_BUILD_JOBS", "1")
            .env("CARGO_TARGET_DIR", root.join("target"))
            .env_remove("CARGO_BUILD_TARGET"),
    )?;
    regular(root, &executable)?;
    Ok(executable)
}

fn stage(
    root: &Path,
    directory: &Path,
    platform: Platform,
    metadata: &Metadata,
    input_binary: &Path,
) -> Result<(), String> {
    build_tree(root, directory)?;
    Metadata::new(
        &metadata.version,
        &metadata.commit,
        metadata.dirty,
        &metadata.build_number,
    )?;
    regular(root, &root.join("Cargo.toml"))?;
    regular(root, &root.join("crates/processor/Cargo.toml"))?;
    let mut files: Vec<(PathBuf, PathBuf)> = ROOT_FILES
        .iter()
        .map(|name| (root.join(name), PathBuf::from(name)))
        .collect();
    files.push((
        root.join("crates/processor/THIRD_PARTY_LICENSES.txt"),
        PathBuf::from("THIRD_PARTY_LICENSES.txt"),
    ));
    files.push((
        root.join("crates/processor/assets/templates.vault"),
        PathBuf::from("templates.vault"),
    ));
    for name in FONTS {
        files.push((
            root.join("crates/processor/licenses/epaint_default_fonts")
                .join(name),
            PathBuf::from("licenses/epaint_default_fonts").join(name),
        ));
    }
    let licenses = root.join("rulesets/Sources/licenses");
    no_links(root, &licenses)?;
    let mut upstream = context(fs::read_dir(&licenses), &licenses)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Cannot list license inputs: {error}"))?;
    upstream.sort_by_key(std::fs::DirEntry::file_name);
    for entry in upstream {
        let path = entry.path();
        if path.extension().is_some_and(|extension| extension == "txt")
            || matches!(
                path.file_name().and_then(|name| name.to_str()),
                Some("README.md" | "README.zh-CN.md")
            )
        {
            files.push((path, PathBuf::from("licenses").join(entry.file_name())));
        }
    }
    if files.iter().all(|(_, destination)| {
        !destination.starts_with("licenses")
            || destination.starts_with("licenses/epaint_default_fonts")
    }) {
        return Err("No upstream license notices were found.".to_owned());
    }
    for (source, _) in &files {
        regular(root, source)?;
    }
    regular(root, input_binary)?;
    let mut header = [0; 9];
    let vault = root.join("crates/processor/assets/templates.vault");
    context(
        context(fs::File::open(&vault), &vault)?.read_exact(&mut header),
        &vault,
    )?;
    if &header[..8] != b"SHVAULT\0" || header[8] != 1 {
        return Err("The bundled template library is not a supported encrypted vault.".to_owned());
    }
    let plist = if platform == Platform::MacOs {
        let path = root.join("crates/processor/assets/Info.plist.in");
        regular(root, &path)?;
        let template = context(fs::read_to_string(&path), &path)?;
        if !template.contains(IDENTIFIER)
            || !template.contains("@VERSION@")
            || !template.contains("@BUILD_NUMBER@")
        {
            return Err("The application identity/version template is invalid.".to_owned());
        }
        Some(
            template
                .replace("@VERSION@", &metadata.version)
                .replace("@BUILD_NUMBER@", &metadata.build_number),
        )
    } else {
        None
    };
    if fs::symlink_metadata(directory).is_ok() {
        return Err("Package staging destination already exists.".to_owned());
    }
    create_directory(root, directory.parent().ok_or("Staging has no parent")?)?;
    context(fs::create_dir(directory), directory)?;
    let destination_resources = directory.join(resources(platform));
    for (source, relative) in files {
        copy_checked(root, &source, &destination_resources.join(relative))?;
    }
    let destination_binary = if platform == Platform::MacOs {
        directory.join("ScriptHub.app/Contents/MacOS/script-hub-gui")
    } else {
        directory.join(binary(platform))
    };
    copy_checked(root, input_binary, &destination_binary)?;
    if let Some(plist) = plist {
        context(
            fs::write(directory.join("ScriptHub.app/Contents/Info.plist"), plist),
            directory,
        )?;
    }
    context(
        fs::write(destination_resources.join("SOURCE.txt"), metadata.source()),
        &destination_resources,
    )?;
    Ok(())
}

fn owned_delivery(root: &Path, platform: Platform) -> Result<bool, String> {
    let mut present = 0;
    for relative in entries(platform) {
        let path = root.join(relative);
        no_links(root, &path)?;
        match fs::symlink_metadata(&path) {
            Ok(_) => present += 1,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("Cannot inspect existing application: {error}")),
        }
    }
    if present == 0 {
        return Ok(false);
    }
    if present != entries(platform).len() {
        return Err(
            "An incomplete existing application must be reviewed before replacement.".to_owned(),
        );
    }
    let marker = root.join(resources(platform)).join("SOURCE.txt");
    regular(root, &marker)?;
    let source = context(fs::read_to_string(&marker), &marker)?;
    if !source
        .lines()
        .any(|line| line.starts_with(&format!("Source: {REPOSITORY}/tree/")))
    {
        return Err(
            "Existing output is not a recognized Script Hub application; it was not replaced."
                .to_owned(),
        );
    }
    let executable = if platform == Platform::MacOs {
        root.join("ScriptHub.app/Contents/MacOS/script-hub-gui")
    } else {
        root.join(binary(platform))
    };
    regular(root, &executable)?;
    if platform == Platform::MacOs {
        let plist = root.join("ScriptHub.app/Contents/Info.plist");
        regular(root, &plist)?;
        if !context(fs::read_to_string(&plist), &plist)?.contains(IDENTIFIER) {
            return Err("Existing application identity does not match Script Hub.".to_owned());
        }
    }
    Ok(true)
}

fn nonce() -> String {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    format!(
        "{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )
}

fn publish_with(
    root: &Path,
    directory: &Path,
    platform: Platform,
    mut verify: impl FnMut(&Path) -> Result<(), String>,
) -> Result<Delivery, String> {
    build_tree(root, directory)?;
    if !owned_delivery(directory, platform)? {
        return Err("No verified application is available in staging.".to_owned());
    }
    verify(directory)?;
    let previous = owned_delivery(root, platform)?;
    let backup = if previous {
        Some(root.join("target/app-backups").join(nonce()))
    } else {
        None
    };
    if let Some(backup) = &backup {
        create_directory(root, backup)?;
    }
    let mut saved = Vec::new();
    let mut installed = Vec::new();
    let result = (|| {
        if let Some(backup) = &backup {
            for entry in entries(platform) {
                context(fs::rename(root.join(entry), backup.join(entry)), root)?;
                saved.push(*entry);
            }
        }
        for entry in entries(platform) {
            context(fs::rename(directory.join(entry), root.join(entry)), root)?;
            installed.push(*entry);
        }
        verify(root)
    })();
    if let Err(error) = result {
        let mut rollback_errors = Vec::new();
        for entry in installed.iter().rev() {
            if let Err(failure) = fs::rename(root.join(entry), directory.join(entry)) {
                rollback_errors.push(failure.to_string());
            }
        }
        if let Some(backup) = &backup {
            for entry in saved.iter().rev() {
                if let Err(failure) = fs::rename(backup.join(entry), root.join(entry)) {
                    rollback_errors.push(failure.to_string());
                }
            }
        }
        return Err(if rollback_errors.is_empty() {
            format!("Delivery failed; previous application restored: {error}")
        } else {
            format!(
                "Delivery failed: {error}. Recovery needs review; backup: {:?}; rollback errors: {}",
                backup,
                rollback_errors.join("; ")
            )
        });
    }
    let destination = root.join(if platform == Platform::MacOs {
        "ScriptHub.app"
    } else {
        binary(platform)
    });
    Ok(Delivery {
        destination,
        backup,
    })
}

fn checked(command: &mut Command) -> Result<(), String> {
    let status = command.status().map_err(|error| {
        format!(
            "Cannot run {}: {error}",
            command.get_program().to_string_lossy()
        )
    })?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "{} exited with {status}",
            command.get_program().to_string_lossy()
        ))
    }
}

fn output(command: &mut Command) -> Result<String, String> {
    let result = command.output().map_err(|error| {
        format!(
            "Cannot run {}: {error}",
            command.get_program().to_string_lossy()
        )
    })?;
    if !result.status.success() {
        return Err(format!(
            "{} exited with {}: {}",
            command.get_program().to_string_lossy(),
            result.status,
            String::from_utf8_lossy(&result.stderr[..result.stderr.len().min(4096)]).trim()
        ));
    }
    String::from_utf8(result.stdout)
        .map_err(|_| "Build tool returned non-UTF-8 metadata.".to_owned())
}

fn verify_package(directory: &Path, platform: Platform, metadata: &Metadata) -> Result<(), String> {
    let executable = if platform == Platform::MacOs {
        directory.join("ScriptHub.app/Contents/MacOS/script-hub-gui")
    } else {
        directory.join(binary(platform))
    };
    if output(Command::new(&executable).arg("--version"))?.trim()
        != format!("script-hub-gui {}", metadata.version)
    {
        return Err("Packaged executable version does not match this workspace.".to_owned());
    }
    if platform == Platform::MacOs {
        let plist = directory.join("ScriptHub.app/Contents/Info.plist");
        checked(Command::new("/usr/bin/plutil").arg("-lint").arg(&plist))?;
        for (key, expected) in [
            ("CFBundleIdentifier", IDENTIFIER),
            ("CFBundleExecutable", "script-hub-gui"),
            ("CFBundlePackageType", "APPL"),
            ("CFBundleShortVersionString", &metadata.version),
            ("CFBundleVersion", &metadata.build_number),
        ] {
            if output(
                Command::new("/usr/libexec/PlistBuddy")
                    .arg("-c")
                    .arg(format!("Print :{key}"))
                    .arg(&plist),
            )?
            .trim()
                != expected
            {
                return Err(format!("Packaged application has an invalid {key}."));
            }
        }
        checked(
            Command::new("/usr/bin/codesign")
                .args(["--verify", "--deep", "--strict"])
                .arg(directory.join("ScriptHub.app")),
        )?;
    }
    Ok(())
}

pub fn run(mut arguments: impl Iterator<Item = String>) -> Result<(), String> {
    let mut root = None;
    let mut archive = false;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--root" if root.is_none() => {
                root = Some(PathBuf::from(
                    arguments
                        .next()
                        .filter(|value| !value.starts_with("--"))
                        .ok_or("--root requires a repository path")?,
                ))
            }
            "--archive" if !archive => archive = true,
            "--help" | "-h" => {
                println!(
                    "cargo app [--root <repository>] [--archive]\nBuilds one native release GUI, verifies its resources and publishes it at the repository root. Existing verified applications are backed up under target/app-backups. No signing certificate or notarization is installed."
                );
                return Ok(());
            }
            _ => {
                return Err(
                    "Use cargo app --help; unsupported or repeated build option.".to_owned(),
                );
            }
        }
    }
    let platform = if cfg!(target_os = "macos") {
        Platform::MacOs
    } else if cfg!(target_os = "windows") {
        Platform::Windows
    } else {
        return Err(
            "Native GUI packaging supports macOS and Windows; Linux desktop development is frozen."
                .to_owned(),
        );
    };
    let root = root.unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."));
    let root = context(root.canonicalize(), &root)?;
    regular(&root, &root.join("Cargo.toml"))?;
    regular(&root, &root.join("crates/processor/Cargo.toml"))?;
    let cache = root.join(".cache");
    create_directory(&root, &cache)?;
    let lock_path = cache.join("smart-build.lock");
    no_links(&root, &lock_path)?;
    let lock = context(
        fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path),
        &lock_path,
    )?;
    lock.try_lock()
        .map_err(|error| format!("Another native build may be active: {error}"))?;
    let compiler = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let host = native_host(&output(Command::new(&compiler).arg("-vV"))?, platform)?;
    let input_binary = build_native_with(&root, platform, &host, checked)?;
    let commit = output(
        Command::new("git")
            .current_dir(&root)
            .args(["rev-parse", "HEAD"]),
    )?;
    let dirty = !output(Command::new("git").current_dir(&root).args([
        "status",
        "--porcelain=v1",
        "--untracked-files=normal",
    ]))?
    .trim()
    .is_empty();
    let metadata = Metadata::new(
        env!("CARGO_PKG_VERSION"),
        commit.trim(),
        dirty,
        &std::env::var("GITHUB_RUN_NUMBER").unwrap_or_else(|_| "1".to_owned()),
    )?;
    let directory = root.join("target/release/bundle").join(nonce());
    stage(&root, &directory, platform, &metadata, &input_binary)?;
    if platform == Platform::MacOs {
        checked(
            Command::new("/usr/bin/codesign")
                .args(["--force", "--sign", "-"])
                .arg(directory.join("ScriptHub.app")),
        )?;
    }
    let delivery = publish_with(&root, &directory, platform, |directory| {
        verify_package(directory, platform, &metadata)
    })?;
    context(fs::remove_dir(&directory), &directory)?;
    println!(
        "[SUCCESS] Native application: {}",
        delivery.destination.display()
    );
    if let Some(backup) = delivery.backup {
        println!(
            "[INFO] Previous application preserved: {}",
            backup.display()
        );
    }
    if archive {
        let suffix = if platform == Platform::MacOs {
            "macos.tar.gz"
        } else {
            "windows.zip"
        };
        let destination = root.join(format!(
            "target/script-hub-gui-{}-{suffix}",
            metadata.version
        ));
        no_links(&root, &destination)?;
        let temporary = root.join(format!(
            "target/.script-hub-archive-{}.{}",
            nonce(),
            if platform == Platform::MacOs {
                "tar.gz"
            } else {
                "zip"
            }
        ));
        let mut command = Command::new(if platform == Platform::Windows {
            "tar.exe"
        } else {
            "tar"
        });
        if platform == Platform::MacOs {
            command.arg("-czf");
        } else {
            command.args(["-a", "-c", "-f"]);
        }
        command
            .arg(&temporary)
            .arg("-C")
            .arg(&root)
            .args(entries(platform));
        checked(&mut command)?;
        regular(&root, &temporary)?;
        let previous = match fs::symlink_metadata(&destination) {
            Ok(_) => {
                regular(&root, &destination)?;
                let backup = root.join("target/app-backups").join(nonce());
                create_directory(&root, &backup)?;
                let backup = backup.join(destination.file_name().ok_or("Archive has no filename")?);
                context(fs::rename(&destination, &backup), &destination)?;
                Some(backup)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(format!("Cannot inspect previous archive: {error}")),
        };
        if let Err(error) = fs::rename(&temporary, &destination) {
            if let Some(previous) = &previous {
                context(fs::rename(previous, &destination), previous).map_err(|rollback| format!("Archive delivery failed: {error}. Previous archive requires recovery: {rollback}"))?;
            }
            return Err(format!(
                "Application was delivered, but archive delivery failed: {error}"
            ));
        }
        println!("[SUCCESS] Archive: {}", destination.display());
        if let Some(previous) = previous {
            println!("[INFO] Previous archive preserved: {}", previous.display());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
