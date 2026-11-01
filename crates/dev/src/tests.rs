use super::*;
use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};

const ROOT_FILES: &[&str] = &[
    "CHANGELOG.md",
    "CHANGELOG.zh-CN.md",
    "docs/TEMPLATES.md",
    "docs/TEMPLATES.zh-CN.md",
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
const COMMIT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const MAC_HOST: &str = "aarch64-apple-darwin";
const WINDOWS_HOST: &str = "x86_64-pc-windows-msvc";

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "script-hub-package-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let fixture = Self(root);
        fixture.write(
            "Cargo.toml",
            b"[workspace]\nmembers = [\"crates/processor\"]\n",
        );
        fixture.write(
            "crates/processor/Cargo.toml",
            b"[package]\nname = \"rust_processor\"\n",
        );
        fixture.write("crates/processor/assets/Info.plist.in", b"<plist><dict><key>CFBundleIdentifier</key><string>io.github.nowaytouse.script-hub</string><key>CFBundleShortVersionString</key><string>@VERSION@</string><key>CFBundleVersion</key><string>@BUILD_NUMBER@</string></dict></plist>");
        fixture.write(
            "crates/processor/assets/templates.vault",
            b"SHVAULT\0\x01synthetic ciphertext",
        );
        fixture.write(
            "crates/processor/THIRD_PARTY_LICENSES.txt",
            b"synthetic dependency notice",
        );
        for name in ROOT_FILES {
            fixture.write(name, format!("synthetic {name}").as_bytes());
        }
        for name in FONTS {
            fixture.write(
                &format!("crates/processor/licenses/epaint_default_fonts/{name}"),
                b"synthetic font notice",
            );
        }
        for name in [
            "AdGuard-CNAME-MIT.txt",
            "app2smile-MIT.txt",
            "README.md",
            "README.zh-CN.md",
        ] {
            fixture.write(
                &format!("rulesets/Sources/licenses/{name}"),
                b"synthetic upstream notice",
            );
        }
        fixture.write(
            "target/release/script-hub-gui",
            b"synthetic native executable",
        );
        fixture.write(
            "target/release/script-hub-gui.exe",
            b"synthetic native executable",
        );
        fixture.write(
            &format!("target/{MAC_HOST}/release/script-hub-gui"),
            b"synthetic native executable",
        );
        fixture.write(
            &format!("target/{WINDOWS_HOST}/release/script-hub-gui.exe"),
            b"synthetic native executable",
        );
        fixture
    }
    fn write(&self, relative: &str, bytes: &[u8]) {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
    fn metadata() -> Metadata {
        Metadata::new("0.2.0", COMMIT, false, "7").unwrap()
    }
    fn stage(&self) -> PathBuf {
        self.0.join("target/release/bundle/fixture-stage")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn stage(
    root: &Path,
    directory: &Path,
    platform: Platform,
    metadata: &Metadata,
) -> Result<(), String> {
    let host = if platform == Platform::MacOs {
        MAC_HOST
    } else {
        WINDOWS_HOST
    };
    super::stage(
        root,
        directory,
        platform,
        metadata,
        &root
            .join("target")
            .join(host)
            .join("release")
            .join(binary(platform)),
    )
}

#[test]
fn metadata_rejects_xml_or_source_marker_injection() {
    assert!(Metadata::new("0.2.0", COMMIT, false, "7").is_ok());
    for (version, commit, number) in [
        ("0.2.0</string>", COMMIT, "7"),
        ("0.2.0", "not-a-commit", "7"),
        ("0.2.0", COMMIT, "7\nprivate"),
    ] {
        assert!(Metadata::new(version, commit, false, number).is_err());
    }
}

#[test]
fn native_packages_keep_public_template_help_and_release_notes() {
    for (platform, directory) in [
        (Platform::MacOs, "ScriptHub.app/Contents/Resources"),
        (Platform::Windows, "ScriptHub.resources"),
    ] {
        let fixture = Fixture::new();
        stage(&fixture.0, &fixture.stage(), platform, &Fixture::metadata()).unwrap();
        for (relative, expected) in [
            ("docs/TEMPLATES.md", "synthetic docs/TEMPLATES.md"),
            (
                "docs/TEMPLATES.zh-CN.md",
                "synthetic docs/TEMPLATES.zh-CN.md",
            ),
            ("CHANGELOG.md", "synthetic CHANGELOG.md"),
            ("CHANGELOG.zh-CN.md", "synthetic CHANGELOG.zh-CN.md"),
        ] {
            let path = fixture.stage().join(directory).join(relative);
            assert!(
                path.is_file(),
                "Public help is missing from the native package"
            );
            assert_eq!(fs::read_to_string(path).unwrap(), expected);
        }
    }
}

#[test]
fn macos_stage_is_complete_and_does_not_publish_early() {
    let fixture = Fixture::new();
    stage(
        &fixture.0,
        &fixture.stage(),
        Platform::MacOs,
        &Fixture::metadata(),
    )
    .unwrap();
    let app = fixture.stage().join("ScriptHub.app/Contents");
    assert_eq!(
        fs::read(app.join("MacOS/script-hub-gui")).unwrap(),
        b"synthetic native executable"
    );
    assert_eq!(
        fs::read(app.join("Resources/templates.vault")).unwrap(),
        b"SHVAULT\0\x01synthetic ciphertext"
    );
    assert!(
        fs::read_to_string(app.join("Resources/SOURCE.txt"))
            .unwrap()
            .contains(COMMIT)
    );
    let plist = fs::read_to_string(app.join("Info.plist")).unwrap();
    assert!(plist.contains("0.2.0") && !plist.contains('@'));
    assert!(!fixture.0.join("ScriptHub.app").exists());
}

#[test]
fn windows_stage_keeps_sidecars_in_one_resources_directory() {
    let fixture = Fixture::new();
    stage(
        &fixture.0,
        &fixture.stage(),
        Platform::Windows,
        &Fixture::metadata(),
    )
    .unwrap();
    assert!(fixture.stage().join("script-hub-gui.exe").is_file());
    assert_eq!(
        fs::read(fixture.stage().join("ScriptHub.resources/templates.vault")).unwrap(),
        b"SHVAULT\0\x01synthetic ciphertext"
    );
    assert!(
        fixture
            .stage()
            .join("ScriptHub.resources/licenses/epaint_default_fonts/OFL.txt")
            .is_file()
    );
    assert!(!fixture.stage().join("templates.vault").exists());
}

#[test]
fn missing_required_resource_preserves_old_delivery() {
    let fixture = Fixture::new();
    fixture.write("ScriptHub.app/keep-user-version", b"original");
    fs::remove_file(fixture.0.join("crates/processor/assets/templates.vault")).unwrap();
    assert!(
        stage(
            &fixture.0,
            &fixture.stage(),
            Platform::MacOs,
            &Fixture::metadata()
        )
        .is_err()
    );
    assert_eq!(
        fs::read(fixture.0.join("ScriptHub.app/keep-user-version")).unwrap(),
        b"original"
    );
    assert!(!fixture.stage().exists());
}

#[test]
fn unowned_root_target_is_never_replaced() {
    let fixture = Fixture::new();
    stage(
        &fixture.0,
        &fixture.stage(),
        Platform::MacOs,
        &Fixture::metadata(),
    )
    .unwrap();
    fixture.write("ScriptHub.app/user-file", b"private local data");
    assert!(publish_with(&fixture.0, &fixture.stage(), Platform::MacOs, |_| Ok(())).is_err());
    assert_eq!(
        fs::read(fixture.0.join("ScriptHub.app/user-file")).unwrap(),
        b"private local data"
    );
}

#[test]
fn verified_replacement_preserves_a_recoverable_previous_app() {
    let fixture = Fixture::new();
    stage(
        &fixture.0,
        &fixture.stage(),
        Platform::MacOs,
        &Fixture::metadata(),
    )
    .unwrap();
    publish_with(&fixture.0, &fixture.stage(), Platform::MacOs, |_| Ok(())).unwrap();
    fixture.write(
        "ScriptHub.app/Contents/Resources/user-preserved",
        b"old marker",
    );
    let next = fixture.0.join("target/release/bundle/second-stage");
    stage(&fixture.0, &next, Platform::MacOs, &Fixture::metadata()).unwrap();
    let delivery = publish_with(&fixture.0, &next, Platform::MacOs, |_| Ok(())).unwrap();
    assert_eq!(delivery.destination, fixture.0.join("ScriptHub.app"));
    assert_eq!(
        fs::read(
            delivery
                .backup
                .unwrap()
                .join("ScriptHub.app/Contents/Resources/user-preserved")
        )
        .unwrap(),
        b"old marker"
    );
}

#[test]
fn failed_post_publish_verification_rolls_back_the_previous_app() {
    let fixture = Fixture::new();
    stage(
        &fixture.0,
        &fixture.stage(),
        Platform::MacOs,
        &Fixture::metadata(),
    )
    .unwrap();
    publish_with(&fixture.0, &fixture.stage(), Platform::MacOs, |_| Ok(())).unwrap();
    fixture.write("ScriptHub.app/Contents/Resources/previous", b"keep");
    let next = fixture.0.join("target/release/bundle/next-stage");
    stage(&fixture.0, &next, Platform::MacOs, &Fixture::metadata()).unwrap();
    let result = publish_with(&fixture.0, &next, Platform::MacOs, |directory| {
        if directory == fixture.0 {
            Err("synthetic verification failure".to_owned())
        } else {
            Ok(())
        }
    });
    assert!(result.is_err());
    assert_eq!(
        fs::read(fixture.0.join("ScriptHub.app/Contents/Resources/previous")).unwrap(),
        b"keep"
    );
}

#[test]
fn windows_post_publish_failure_restores_both_binary_and_resources() {
    let fixture = Fixture::new();
    stage(
        &fixture.0,
        &fixture.stage(),
        Platform::Windows,
        &Fixture::metadata(),
    )
    .unwrap();
    publish_with(&fixture.0, &fixture.stage(), Platform::Windows, |_| Ok(())).unwrap();
    fixture.write("script-hub-gui.exe", b"previous executable");
    fixture.write("ScriptHub.resources/previous", b"previous resources");
    let next = fixture.0.join("target/release/bundle/next-stage");
    stage(&fixture.0, &next, Platform::Windows, &Fixture::metadata()).unwrap();
    let result = publish_with(&fixture.0, &next, Platform::Windows, |directory| {
        if directory == fixture.0 {
            Err("synthetic paired verification failure".to_owned())
        } else {
            Ok(())
        }
    });
    assert!(result.is_err());
    assert_eq!(
        fs::read(fixture.0.join("script-hub-gui.exe")).unwrap(),
        b"previous executable"
    );
    assert_eq!(
        fs::read(fixture.0.join("ScriptHub.resources/previous")).unwrap(),
        b"previous resources"
    );
    assert_eq!(
        fs::read(next.join("script-hub-gui.exe")).unwrap(),
        b"synthetic native executable"
    );
    assert!(!next.join("ScriptHub.resources/previous").exists());
}

#[cfg(unix)]
#[test]
fn symlinked_resources_and_root_targets_are_rejected() {
    let fixture = Fixture::new();
    let vault = fixture.0.join("crates/processor/assets/templates.vault");
    fs::remove_file(&vault).unwrap();
    std::os::unix::fs::symlink(fixture.0.join("LICENSE"), &vault).unwrap();
    assert!(
        stage(
            &fixture.0,
            &fixture.stage(),
            Platform::MacOs,
            &Fixture::metadata()
        )
        .is_err()
    );
    fs::remove_file(&vault).unwrap();
    fs::write(&vault, b"SHVAULT\0\x01synthetic ciphertext").unwrap();
    stage(
        &fixture.0,
        &fixture.stage(),
        Platform::MacOs,
        &Fixture::metadata(),
    )
    .unwrap();
    std::os::unix::fs::symlink(fixture.0.join("crates"), fixture.0.join("ScriptHub.app")).unwrap();
    assert!(publish_with(&fixture.0, &fixture.stage(), Platform::MacOs, |_| Ok(())).is_err());
    assert!(fixture.0.join("crates/processor/Cargo.toml").is_file());
}

#[test]
fn stage_cannot_escape_the_owned_build_tree() {
    let fixture = Fixture::new();
    assert!(
        stage(
            &fixture.0,
            &fixture.0.join("modules/unsafe-stage"),
            Platform::MacOs,
            &Fixture::metadata()
        )
        .is_err()
    );
    assert!(!fixture.0.join("modules/unsafe-stage").exists());
}

#[test]
fn compiler_host_must_be_native_and_one_safe_path_component() {
    assert_eq!(
        native_host(
            &format!("rustc 1.91.0\nhost: {MAC_HOST}\n"),
            Platform::MacOs
        )
        .unwrap(),
        MAC_HOST
    );
    assert_eq!(
        native_host(&format!("host: {WINDOWS_HOST}\n"), Platform::Windows).unwrap(),
        WINDOWS_HOST
    );
    for verbose in [
        "host: ../../outside",
        "host: x86_64-unknown-linux-gnu",
        "host: aarch64-apple-darwin\nhost: aarch64-apple-darwin",
        "release: 1.91.0",
    ] {
        assert!(native_host(verbose, Platform::MacOs).is_err(), "{verbose}");
    }
}

#[test]
fn native_build_selects_explicit_host_and_not_stale_flat_output() {
    let fixture = Fixture::new();
    let executable = build_native_with(&fixture.0, Platform::MacOs, MAC_HOST, |command| {
        let arguments: Vec<_> = command
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect();
        assert!(
            arguments
                .windows(2)
                .any(|pair| pair == ["--target", MAC_HOST])
        );
        assert!(arguments.contains(&"--locked".to_owned()));
        fixture.write(
            &format!("target/{MAC_HOST}/release/script-hub-gui"),
            b"fresh native executable",
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(
        executable,
        fixture
            .0
            .join(format!("target/{MAC_HOST}/release/script-hub-gui"))
    );
    super::stage(
        &fixture.0,
        &fixture.stage(),
        Platform::MacOs,
        &Fixture::metadata(),
        &executable,
    )
    .unwrap();
    assert_eq!(
        fs::read(
            fixture
                .stage()
                .join("ScriptHub.app/Contents/MacOS/script-hub-gui")
        )
        .unwrap(),
        b"fresh native executable"
    );
}

#[cfg(unix)]
#[test]
fn symlinked_target_is_rejected_before_build_invocation() {
    let fixture = Fixture::new();
    fs::rename(fixture.0.join("target"), fixture.0.join("preserved-target")).unwrap();
    std::os::unix::fs::symlink(fixture.0.join("preserved-target"), fixture.0.join("target"))
        .unwrap();
    let mut invoked = false;
    let result = build_native_with(&fixture.0, Platform::MacOs, MAC_HOST, |_| {
        invoked = true;
        Ok(())
    });
    assert!(result.is_err());
    assert!(!invoked);
    assert_eq!(
        fs::read(fixture.0.join("preserved-target/release/script-hub-gui")).unwrap(),
        b"synthetic native executable"
    );
}

#[cfg(unix)]
#[test]
fn symlinked_native_binary_ancestor_is_rejected_before_build() {
    let fixture = Fixture::new();
    fs::rename(
        fixture.0.join(format!("target/{MAC_HOST}/release")),
        fixture
            .0
            .join(format!("target/{MAC_HOST}/preserved-release")),
    )
    .unwrap();
    std::os::unix::fs::symlink(
        fixture.0.join("target/release"),
        fixture.0.join(format!("target/{MAC_HOST}/release")),
    )
    .unwrap();
    let mut invoked = false;
    assert!(
        build_native_with(&fixture.0, Platform::MacOs, MAC_HOST, |_| {
            invoked = true;
            Ok(())
        })
        .is_err()
    );
    assert!(!invoked);
}
