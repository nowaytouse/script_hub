//! Local project discovery, template selection and protected destinations.

use crate::runtime::job::validate_root;
pub(crate) use crate::vault::destination::{
    safe_export_destination, writable_template_destination,
};
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
}
