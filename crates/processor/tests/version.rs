use std::process::Command;

#[test]
fn standalone_version_requests_share_the_cargo_version_without_starting_work() {
    for (binary, path) in [
        ("update", env!("CARGO_BIN_EXE_update")),
        ("validate", env!("CARGO_BIN_EXE_validate")),
        ("guard", env!("CARGO_BIN_EXE_guard")),
        ("promax", env!("CARGO_BIN_EXE_promax")),
    ] {
        for flag in ["--version", "-V"] {
            let output = Command::new(path).arg(flag).output().unwrap();
            assert!(output.status.success(), "{binary}: {output:?}");
            assert_eq!(
                String::from_utf8(output.stdout).unwrap().trim(),
                format!("{binary} {}", env!("CARGO_PKG_VERSION"))
            );
        }
    }
}

#[test]
fn offline_index_refresh_rejects_execution_before_accessing_the_workspace() {
    let output = Command::new(env!("CARGO_BIN_EXE_promax"))
        .args([
            "--root",
            "missing-repository",
            "--refresh-index",
            "--execute",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("cannot be combined")
    );
}

#[test]
fn missing_root_values_do_not_consume_the_offline_flag_as_a_path() {
    for arguments in [vec!["--root", "--refresh-index"], vec!["--root", ""]] {
        let output = Command::new(env!("CARGO_BIN_EXE_promax"))
            .args(arguments)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("Missing value")
        );
    }
}
