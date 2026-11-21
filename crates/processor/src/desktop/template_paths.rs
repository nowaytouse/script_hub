//! Local project discovery, template selection and protected destinations.

use crate::runtime::job::validate_root;
use crate::vault::{self, Client};
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

pub(crate) fn resolve_project_root(
    explicit: Option<&Path>,
    remembered: Option<&Path>,
    executable: Option<&Path>,
    current_directory: Option<&Path>,
) -> Result<Option<PathBuf>, String> {
    if let Some(path) = explicit {
        return validate_root(path).map(Some);
    }
    let installed = executable
        .filter(|path| {
            matches!(
                path.file_name().and_then(|name| name.to_str()),
                Some("script-hub-gui" | "script-hub-gui.exe")
            )
        })
        .and_then(Path::parent)
        .and_then(|directory| {
            directory
                .ancestors()
                .find_map(|path| validate_root(path).ok())
        });
    Ok(installed
        .or_else(|| current_directory.and_then(|path| validate_root(path).ok()))
        .or_else(|| {
            remembered
                .filter(|path| path.is_absolute())
                .and_then(|path| validate_root(path).ok())
        }))
}

pub(crate) fn parse_template_setup(
    args: impl Iterator<Item = String>,
) -> Option<(Vec<PathBuf>, PathBuf, PathBuf)> {
    let mut paths: Vec<_> = args.map(PathBuf::from).collect();
    if !(3..=crate::vault::MAX_TEMPLATES + 2).contains(&paths.len())
        || paths
            .iter()
            .any(|path| path.as_os_str().is_empty() || path.to_string_lossy().starts_with('-'))
    {
        return None;
    }
    let public = paths.remove(0);
    let backup = paths.remove(0);
    if public == backup
        || paths
            .iter()
            .any(|source| source == &public || source == &backup)
    {
        return None;
    }
    Some((paths, public, backup))
}

#[cfg(any(target_os = "windows", test))]
fn windows_resource_directory(executable: &Path) -> Option<PathBuf> {
    Some(executable.parent()?.join("ScriptHub.resources"))
}

pub(crate) fn bundled_vault(executable: &Path) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        Some(
            executable
                .parent()?
                .parent()?
                .join("Resources/templates.vault"),
        )
    }
    #[cfg(not(target_os = "macos"))]
    {
        Some(windows_resource_directory(executable)?.join("templates.vault"))
    }
}

pub(crate) fn writable_template_destination(path: &Path) -> bool {
    std::env::current_exe()
        .ok()
        .is_some_and(|executable| outside_application_assets(path, &executable))
}

fn outside_application_assets(path: &Path, executable: &Path) -> bool {
    #[cfg(target_os = "windows")]
    let resources = {
        let Some(resources) = windows_resource_directory(executable) else {
            return false;
        };
        Some(resources)
    };
    #[cfg(not(target_os = "windows"))]
    let resources: Option<PathBuf> = None;
    outside_application_assets_with_resources(path, executable, resources.as_deref())
}

fn outside_application_assets_with_resources(
    path: &Path,
    executable: &Path,
    windows_resources: Option<&Path>,
) -> bool {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        let Ok(directory) = std::env::current_dir() else {
            return false;
        };
        directory.join(path)
    };
    let Some(parent) = absolute.parent() else {
        return false;
    };
    let mut existing = parent;
    let resolved = loop {
        match existing.canonicalize() {
            Ok(resolved) => break resolved,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if absolute
                    .components()
                    .any(|part| part == std::path::Component::ParentDir)
                {
                    return false;
                }
                let Some(ancestor) = existing.parent() else {
                    return false;
                };
                existing = ancestor;
            }
            Err(_) => return false,
        }
    };
    if let Some(resources) = windows_resources
        && !matches!(
            protected_windows_resources(&absolute, &resolved, resources),
            Ok(false)
        )
    {
        return false;
    }
    // Inspect resolved aliases and declared bundle ancestors, including missing child directories.
    let declared = (!absolute
        .components()
        .any(|part| part == std::path::Component::ParentDir))
    .then_some(parent);
    for ancestor in resolved
        .ancestors()
        .chain(declared.into_iter().flat_map(Path::ancestors))
    {
        if !ancestor
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("app"))
        {
            continue;
        }
        match application_bundle(ancestor) {
            Ok(false) => {}
            Ok(true) | Err(_) => return false,
        }
    }
    let normalized_destination = (existing == parent)
        .then(|| absolute.file_name().map(|name| resolved.join(name)))
        .flatten();
    if let Some(asset) = bundled_vault(executable).and_then(|asset| {
        Some(
            asset
                .parent()?
                .canonicalize()
                .ok()?
                .join(asset.file_name()?),
        )
    }) {
        let protected = normalized_destination.as_ref().is_some_and(|destination| {
            #[cfg(target_os = "windows")]
            {
                destination == &asset
                    || (destination.parent() == asset.parent()
                        && destination
                            .file_name()
                            .and_then(|name| name.to_str())
                            .zip(asset.file_name().and_then(|name| name.to_str()))
                            .is_some_and(|(left, right)| left.eq_ignore_ascii_case(right)))
            }
            #[cfg(not(target_os = "windows"))]
            {
                destination == &asset
            }
        });
        if protected {
            return false;
        }
    }
    true
}

fn protected_windows_resources(
    destination: &Path,
    resolved_parent: &Path,
    resources: &Path,
) -> std::io::Result<bool> {
    let Some(resource_parent) = resources.parent() else {
        return Ok(true);
    };
    let Some(resource_name) = resources.file_name() else {
        return Ok(true);
    };
    let resource_parent = resource_parent.canonicalize()?;
    // Protect the declared package namespace even when it is absent or aliases elsewhere.
    for ancestor in destination.ancestors() {
        if !ancestor.file_name().is_some_and(|name| {
            name.as_encoded_bytes()
                .eq_ignore_ascii_case(resource_name.as_encoded_bytes())
        }) {
            continue;
        }
        let Some(parent) = ancestor.parent() else {
            return Ok(true);
        };
        match parent.canonicalize() {
            Ok(parent) if parent == resource_parent => return Ok(true),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    let canonical_resources = match resources.canonicalize() {
        Ok(resources) => resources,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if resolved_parent.starts_with(&canonical_resources) {
        return Ok(true);
    }
    match destination.canonicalize() {
        Ok(destination) => Ok(destination.starts_with(canonical_resources)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn application_bundle(path: &Path) -> std::io::Result<bool> {
    let info = match std::fs::metadata(path.join("Contents/Info.plist")) {
        Ok(info) => info,
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) =>
        {
            return Ok(false);
        }
        Err(error) => return Err(error),
    };
    let executables = match std::fs::metadata(path.join("Contents/MacOS")) {
        Ok(executables) => executables,
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) =>
        {
            return Ok(false);
        }
        Err(error) => return Err(error),
    };
    Ok(info.is_file() && executables.is_dir())
}

pub(crate) fn inferred_client(path: &Path) -> Client {
    let filename = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    if filename.contains("shadowrocket") {
        return Client::Shadowrocket;
    }
    if filename.contains("surge") {
        return Client::Surge;
    }
    let Ok(bytes) = vault::read_bounded(path) else {
        return Client::Other;
    };
    let bytes = Zeroizing::new(bytes);
    if bytes.len() > vault::MAX_TEMPLATE_BYTES {
        return Client::Other;
    }
    match std::str::from_utf8(&bytes) {
        Ok(content) if vault::template::is_flclash_config(content) => Client::FlClash,
        _ => Client::Other,
    }
}

pub(crate) fn configuration_export_path(path: &Path, client: Client) -> Option<PathBuf> {
    let default_extension = match client {
        Client::FlClash => Some("yaml"),
        Client::SingBox => Some("json"),
        _ => None,
    };
    if let Some(extension) = default_extension {
        let filename = path.file_name()?;
        let raw = path.as_os_str().as_encoded_bytes();
        // A verbatim Windows path may treat a trailing slash as filename data.
        if raw
            .last()
            .is_some_and(|&byte| std::path::is_separator(char::from(byte)))
            || !raw.ends_with(filename.as_encoded_bytes())
            || path.is_dir()
        {
            return None;
        }
        if path.extension().is_none() {
            return Some(path.with_extension(extension));
        }
    }
    Some(path.to_owned())
}

pub(crate) fn safe_export_destination(path: &Path, workspace: Option<&Path>) -> bool {
    #[cfg(windows)]
    if path
        .file_name()
        .is_none_or(|name| !unambiguous_windows_export_name(name))
    {
        return false;
    }
    if !writable_template_destination(path)
        || !path.is_absolute()
        || path
            .components()
            .any(|component| component == std::path::Component::ParentDir)
    {
        return false;
    }
    let Some(parent) = path.parent().and_then(|parent| parent.canonicalize().ok()) else {
        return false;
    };
    if workspace.is_some_and(|root| parent.starts_with(root)) {
        return false;
    }
    if parent
        .ancestors()
        .any(|ancestor| ancestor.join(".git").exists())
    {
        return false;
    }
    if let Some(storage) = crate::runtime::settings::preferences_path()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join("vaults")))
        .and_then(|path| path.canonicalize().ok())
        && parent.starts_with(storage)
    {
        return false;
    }
    !path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("vault"))
}

#[cfg(any(windows, test))]
fn unambiguous_windows_export_name(name: &std::ffi::OsStr) -> bool {
    let name = name.as_encoded_bytes();
    // Win32 may strip a terminal dot or space, and a colon selects an NTFS stream.
    !matches!(name.last(), Some(b'.' | b' ')) && !name.contains(&b':')
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "script-hub-template-path-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path.canonicalize().unwrap())
        }

        fn workspace() -> Self {
            let workspace = Self::new();
            for directory in ["crates/processor", "modules", "rulesets/Sources"] {
                std::fs::create_dir_all(workspace.0.join(directory)).unwrap();
            }
            std::fs::write(workspace.0.join("crates/processor/Cargo.toml"), "").unwrap();
            std::fs::write(
                workspace.0.join("rulesets/Sources/adblock_whitelist.list"),
                "",
            )
            .unwrap();
            workspace
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }

    const FLCLASH_FIXTURE: &str = "proxy-providers:\n  subscription:\n    type: http\n    url: \"{{SUBSCRIPTION_1}}\"\n    path: ./providers/subscription.yaml\n    interval: 3600\nproxy-groups:\n  - name: PROXY\n    type: select\n    use: [subscription]\nrules:\n  - MATCH,PROXY\ndns:\n  nameserver: ['https://resolver.example/dns-query']\n  default-nameserver: ['https://192.0.2.53/dns-query']\n";

    #[test]
    fn template_setup_accepts_only_separate_outputs_and_bounded_source_paths() {
        let root = std::env::temp_dir();
        let public = root.join("public.vault");
        let backup = root.join("local.vault");
        let source = root.join("synthetic.conf");
        let args = [&public, &backup, &source].map(|path| path.to_string_lossy().into_owned());
        let (sources, parsed_public, parsed_backup) =
            parse_template_setup(args.into_iter()).unwrap();
        assert_eq!(
            (sources, parsed_public, parsed_backup),
            (vec![source.clone()], public.clone(), backup)
        );
        assert!(
            parse_template_setup(
                [public.clone(), public, source]
                    .into_iter()
                    .map(|path| path.to_string_lossy().into_owned())
            )
            .is_none()
        );
        assert!(
            parse_template_setup(
                [
                    "--password".to_owned(),
                    "not-an-option".to_owned(),
                    "input.conf".to_owned()
                ]
                .into_iter()
            )
            .is_none()
        );
        assert!(parse_template_setup(["only-one-path".to_owned()].into_iter()).is_none());
    }

    #[test]
    fn discovery_prefers_the_apps_project_over_other_projects() {
        let workspace = Fixture::workspace();
        let current_directory = Fixture::workspace();
        let remembered = Fixture::workspace();
        let executable = workspace
            .0
            .join("ScriptHub.app/Contents/MacOS/script-hub-gui");
        assert_eq!(
            resolve_project_root(
                None,
                Some(&remembered.0),
                Some(&executable),
                Some(&current_directory.0),
            )
            .unwrap(),
            Some(workspace.0.canonicalize().unwrap())
        );
    }

    #[test]
    fn a_detached_app_can_use_a_previously_saved_directory() {
        let remembered = Fixture::workspace();
        assert_eq!(
            resolve_project_root(None, Some(&remembered.0), None, None).unwrap(),
            Some(remembered.0.canonicalize().unwrap())
        );
        assert!(
            resolve_project_root(None, Some(&remembered.0.join("missing")), None, None)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn discovery_only_uses_the_current_directory_if_it_is_a_project() {
        let workspace = Fixture::workspace();
        let arbitrary = workspace.0.join("target/debug/unrelated-tool");
        assert_eq!(
            resolve_project_root(None, None, Some(&arbitrary), Some(&workspace.0)).unwrap(),
            Some(workspace.0.canonicalize().unwrap())
        );
        assert!(
            resolve_project_root(
                None,
                None,
                Some(&arbitrary),
                Some(&workspace.0.join("modules"))
            )
            .unwrap()
            .is_none(),
        );
    }

    #[test]
    fn invalid_explicit_project_does_not_use_another_directory() {
        let project = Fixture::workspace();
        let invalid = project.0.join("missing");
        assert!(
            resolve_project_root(Some(&invalid), Some(&project.0), None, Some(&project.0)).is_err()
        );
    }

    #[test]
    fn flclash_detection_reads_supported_content_including_extensionless_files() {
        let fixture = Fixture::new();
        for filename in ["profile.yaml", "profile.yml", "FULL"] {
            let source = fixture.0.join(filename);
            std::fs::write(&source, FLCLASH_FIXTURE).unwrap();
            assert_eq!(inferred_client(&source), Client::FlClash);
        }
        // Existing client name hints still work without reading their contents.
        assert_eq!(inferred_client(Path::new("surge.conf")), Client::Surge);
        assert_eq!(
            inferred_client(Path::new("shadowrocket.conf")),
            Client::Shadowrocket
        );
    }

    #[test]
    fn flclash_detection_rejects_unrelated_invalid_and_oversized_yaml() {
        let fixture = Fixture::new();
        let path = fixture.0.join("FlClash.yaml");
        for content in [
            "name: unrelated application\nversion: 1\n",
            "# proxy-providers: {}\n# proxy-groups: []\n# rules: []\n",
            "proxy-providers: [unterminated\n",
        ] {
            std::fs::write(&path, content).unwrap();
            assert_eq!(inferred_client(&path), Client::Other);
        }
        std::fs::write(&path, "x".repeat(vault::MAX_TEMPLATE_BYTES + 1)).unwrap();
        assert_eq!(inferred_client(&path), Client::Other);
        assert_eq!(
            inferred_client(&fixture.0.join("missing.yaml")),
            Client::Other
        );
    }

    #[test]
    fn export_format_preserves_explicit_names_and_infers_yaml_or_json() {
        for (client, inferred) in [
            (Client::FlClash, "profile.yaml"),
            (Client::SingBox, "profile.json"),
            (Client::Surge, "profile"),
            (Client::Shadowrocket, "profile"),
            (Client::Other, "profile"),
        ] {
            assert_eq!(
                configuration_export_path(Path::new("profile"), client).as_deref(),
                Some(Path::new(inferred))
            );
            assert_eq!(
                configuration_export_path(Path::new("custom.ext"), client).as_deref(),
                Some(Path::new("custom.ext"))
            );
        }
        let fixture = Fixture::new();
        for client in [Client::FlClash, Client::SingBox] {
            assert!(configuration_export_path(&fixture.0, client).is_none());
            assert!(configuration_export_path(Path::new("missing/"), client).is_none());
            #[cfg(windows)]
            for path in [r"\\?\C:\missing/", r"\\?\C:\missing\"] {
                assert!(configuration_export_path(Path::new(path), client).is_none());
            }
        }
    }

    #[test]
    fn plain_exports_reject_relative_and_workspace_paths() {
        let root = std::env::temp_dir().canonicalize().unwrap();
        assert!(!safe_export_destination(Path::new("template.conf"), None));
        assert!(!safe_export_destination(
            &root.join("template.conf"),
            Some(&root)
        ));
        assert!(!safe_export_destination(
            &root.join("templates.vault"),
            None
        ));
        assert!(!safe_export_destination(
            &root.join("templates.VAULT"),
            None
        ));
        assert!(safe_export_destination(&root.join("template.conf"), None));
    }

    #[test]
    fn windows_plain_export_names_exclude_aliases_and_streams() {
        for name in [
            "templates.vault.",
            "templates.vault ",
            "profile.yaml:stream",
        ] {
            assert!(!unambiguous_windows_export_name(std::ffi::OsStr::new(name)));
            #[cfg(windows)]
            assert!(!safe_export_destination(
                &std::env::temp_dir().join(name),
                None
            ));
        }
        assert!(unambiguous_windows_export_name(std::ffi::OsStr::new(
            "配置 01.yaml"
        )));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn plain_exports_reject_application_bundles_but_allow_external_locations() {
        let root = std::env::temp_dir().join(format!(
            "vault-app-assets-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let bundle = root.join("Synthetic.app");
        std::fs::create_dir_all(bundle.join("Contents/MacOS")).unwrap();
        std::fs::create_dir_all(bundle.join("Contents/Resources")).unwrap();
        std::fs::write(bundle.join("Contents/Info.plist"), "synthetic fixture").unwrap();
        let ordinary = root.join("ordinary.app");
        std::fs::create_dir(&ordinary).unwrap();
        let bundle_allowed =
            safe_export_destination(&bundle.join("Contents/Resources/profile.conf"), None);
        let external_allowed = safe_export_destination(&root.join("profile.conf"), None);
        let ordinary_allowed = safe_export_destination(&ordinary.join("profile.conf"), None);
        let executable = bundle.join("Contents/MacOS/script-hub-gui");
        let inside_missing = outside_application_assets(
            &bundle.join("Contents/new/backup/originals.vault"),
            &executable,
        );
        let outside_missing = outside_application_assets(
            &root.join("new-private-backup/originals.vault"),
            &executable,
        );
        let outside_save_as = outside_application_assets(&root.join("editable.vault"), &executable);
        std::fs::remove_dir_all(root).unwrap();
        assert!(
            !bundle_allowed
                && external_allowed
                && ordinary_allowed
                && !inside_missing
                && outside_missing
                && outside_save_as,
            "application resource allowed={bundle_allowed}, external allowed={external_allowed}, ordinary .app directory allowed={ordinary_allowed}"
        );
    }

    #[test]
    fn windows_packaged_vault_uses_the_resource_directory() {
        let root = std::env::temp_dir();
        let executable = root.join("ScriptHub.exe");
        assert_eq!(
            windows_resource_directory(&executable),
            Some(root.join("ScriptHub.resources"))
        );
    }

    #[test]
    fn windows_packaged_resources_protect_the_whole_namespace() {
        let root = std::env::temp_dir().join(format!(
            "vault-windows-resources-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let resources = root.join("ScriptHub.resources");
        let executable = root.join("ScriptHub.exe");
        let protected_paths = [
            resources.clone(),
            resources.join("templates.vault"),
            resources.join("new/backup/profile.conf"),
            root.join("sCrIpThUb.ReSoUrCeS/new/profile.conf"),
        ];
        let allowed_paths = [
            root.join("my-copy.vault"),
            root.join("local-backups/originals.vault"),
            root.join("ScriptHub.resources-copy/profile.conf"),
        ];
        let absent_protected = protected_paths.iter().all(|path| {
            !outside_application_assets_with_resources(path, &executable, Some(&resources))
        });
        std::fs::create_dir(&resources).unwrap();
        let present_protected = protected_paths.iter().all(|path| {
            !outside_application_assets_with_resources(path, &executable, Some(&resources))
        });
        let neighboring_allowed = allowed_paths.iter().all(|path| {
            outside_application_assets_with_resources(path, &executable, Some(&resources))
        });
        std::fs::remove_dir_all(root).unwrap();
        assert!(
            absent_protected,
            "missing packaged resource directories must remain protected"
        );
        assert!(
            present_protected,
            "all packaged resources must remain protected"
        );
        assert!(
            neighboring_allowed,
            "neighboring user destinations must remain writable"
        );
    }

    #[cfg(unix)]
    #[test]
    fn windows_packaged_resources_reject_resolved_and_declared_aliases() {
        let root = std::env::temp_dir().join(format!(
            "vault-windows-aliases-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let resources = root.join("ScriptHub.resources");
        let external = root.join("external");
        std::fs::create_dir_all(&resources).unwrap();
        std::fs::create_dir(&external).unwrap();
        std::fs::write(resources.join("templates.vault"), "synthetic fixture").unwrap();
        std::os::unix::fs::symlink(&resources, root.join("resource-alias")).unwrap();
        std::os::unix::fs::symlink(&external, resources.join("external-alias")).unwrap();
        std::os::unix::fs::symlink(resources.join("templates.vault"), root.join("file-alias"))
            .unwrap();
        let executable = root.join("ScriptHub.exe");
        let protected_paths = [
            root.join("resource-alias/new/profile.conf"),
            resources.join("external-alias/profile.conf"),
            root.join("file-alias"),
        ];
        let protected = protected_paths.iter().all(|path| {
            !outside_application_assets_with_resources(path, &executable, Some(&resources))
        });
        let editable = outside_application_assets_with_resources(
            &external.join("profile.conf"),
            &executable,
            Some(&resources),
        );
        std::fs::remove_dir_all(root).unwrap();
        assert!(
            protected,
            "canonical and declared resource aliases must remain read-only"
        );
        assert!(editable);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_packaged_sidecar_is_read_only_but_a_neighboring_copy_is_editable() {
        let root = std::env::temp_dir().join(format!(
            "vault-sidecar-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let executable = root.join("script-hub-gui.exe");
        let sidecar = bundled_vault(&executable).unwrap();
        assert_eq!(sidecar, root.join("ScriptHub.resources/templates.vault"));
        let protected = outside_application_assets(&sidecar, &executable);
        let protected_uppercase = outside_application_assets(
            &root.join("SCRIPTHUB.RESOURCES/TEMPLATES.VAULT"),
            &executable,
        );
        let protected_missing = outside_application_assets(
            &root.join("ScriptHub.resources/new/backup/profile.conf"),
            &executable,
        );
        let editable = outside_application_assets(&root.join("my-copy.vault"), &executable);
        let missing_backup =
            outside_application_assets(&root.join("local-backups/originals.vault"), &executable);
        std::fs::remove_dir_all(root).unwrap();
        assert!(
            !protected && !protected_uppercase && !protected_missing && editable && missing_backup
        );
    }
}
