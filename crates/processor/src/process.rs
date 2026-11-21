use std::ffi::OsStr;
use std::process::Command;

/// Prepares a non-interactive child while leaving its standard I/O to the caller.
pub(crate) fn background_command(program: impl AsRef<OsStr>) -> Command {
    let command = Command::new(program);
    #[cfg(windows)]
    let command = {
        use std::os::windows::process::CommandExt;

        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut command = command;
        command.creation_flags(CREATE_NO_WINDOW);
        command
    };
    command
}

#[cfg(all(test, windows))]
mod tests {
    use super::background_command;
    use std::io::Write;
    use std::path::PathBuf;
    use std::process::Stdio;

    #[test]
    fn windows_background_command_has_no_console_and_preserves_pipes() {
        let powershell = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let script = r#"
$ErrorActionPreference = 'Stop'
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class ConsoleProbe {
    [DllImport("kernel32.dll")]
    public static extern IntPtr GetConsoleWindow();
}
'@
if ([ConsoleProbe]::GetConsoleWindow() -ne [IntPtr]::Zero) { exit 81 }
if ([Console]::ReadLine() -ne 'input') { exit 82 }
[Console]::WriteLine('output')
[Console]::Error.WriteLine('diagnostic')
"#;
        let mut child = background_command(powershell)
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                script,
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(b"input\n").unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "background probe failed: {output:?}"
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "output");
        assert_eq!(String::from_utf8_lossy(&output.stderr).trim(), "diagnostic");
    }
}
