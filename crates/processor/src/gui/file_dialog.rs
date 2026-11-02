use std::path::PathBuf;

#[derive(Clone, Copy)]
pub enum FileAction {
    Import,
    ExportBackup,
    ExportNodes,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn choose(action: FileAction) -> Result<Option<PathBuf>, ()> {
    use std::{
        io::Read,
        process::{Command, Stdio},
    };

    #[cfg(target_os = "macos")]
    let mut command = {
        let script = match action {
            FileAction::Import => {
                "try\nPOSIX path of (choose file)\non error number -128\nreturn \"\"\nend try"
            }
            FileAction::ExportBackup => {
                "try\nPOSIX path of (choose file name default name \"relay-backup.json\")\non error number -128\nreturn \"\"\nend try"
            }
            FileAction::ExportNodes => {
                "try\nPOSIX path of (choose file name default name \"nodes.list\")\non error number -128\nreturn \"\"\nend try"
            }
        };
        let mut command = Command::new("/usr/bin/osascript");
        command.args(["-e", script]);
        command
    };
    #[cfg(target_os = "windows")]
    let mut command = {
        use std::os::windows::process::CommandExt;
        let script = match action {
            FileAction::Import => {
                "$ErrorActionPreference='Stop'; Add-Type -AssemblyName System.Windows.Forms; [Console]::OutputEncoding=[System.Text.UTF8Encoding]::new($false); $d=New-Object System.Windows.Forms.OpenFileDialog; $d.Filter='JSON files (*.json)|*.json|Text files (*.txt;*.list)|*.txt;*.list|All files (*.*)|*.*'; if ($d.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) { [Console]::Write($d.FileName) }; $d.Dispose()"
            }
            FileAction::ExportBackup => {
                "$ErrorActionPreference='Stop'; Add-Type -AssemblyName System.Windows.Forms; [Console]::OutputEncoding=[System.Text.UTF8Encoding]::new($false); $d=New-Object System.Windows.Forms.SaveFileDialog; $d.FileName='relay-backup.json'; $d.Filter='JSON files (*.json)|*.json|All files (*.*)|*.*'; if ($d.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) { [Console]::Write($d.FileName) }; $d.Dispose()"
            }
            FileAction::ExportNodes => {
                "$ErrorActionPreference='Stop'; Add-Type -AssemblyName System.Windows.Forms; [Console]::OutputEncoding=[System.Text.UTF8Encoding]::new($false); $d=New-Object System.Windows.Forms.SaveFileDialog; $d.FileName='nodes.list'; $d.Filter='Text files (*.txt;*.list)|*.txt;*.list|All files (*.*)|*.*'; if ($d.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) { [Console]::Write($d.FileName) }; $d.Dispose()"
            }
        };
        let root = std::path::PathBuf::from(std::env::var_os("SystemRoot").ok_or(())?);
        if !root.is_absolute() {
            return Err(());
        }
        let mut command = Command::new(root.join("System32/WindowsPowerShell/v1.0/powershell.exe"));
        command
            .args(["-NoProfile", "-STA", "-Command", script])
            .creation_flags(0x08000000);
        command
    };
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| ())?;
    let mut output = Vec::new();
    let read = child
        .stdout
        .take()
        .ok_or(())?
        .take(32 * 1024 + 1)
        .read_to_end(&mut output);
    if read.is_err() || output.len() > 32 * 1024 {
        let _ = child.kill();
        let _ = child.wait();
        return Err(());
    }
    if !child.wait().map_err(|_| ())?.success() {
        return Err(());
    }
    parse_response(&output)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn choose(_action: FileAction) -> Result<Option<PathBuf>, ()> {
    Err(())
}

#[cfg(any(test, target_os = "macos", target_os = "windows"))]
fn parse_response(bytes: &[u8]) -> Result<Option<PathBuf>, ()> {
    if bytes.len() > 32 * 1024 {
        return Err(());
    }
    let output = std::str::from_utf8(bytes).map_err(|_| ())?;
    #[cfg(not(target_os = "windows"))]
    let output = output.strip_suffix('\n').unwrap_or(output);
    if output.is_empty() {
        return Ok(None);
    }
    if output.chars().any(char::is_control) {
        return Err(());
    }
    let path = PathBuf::from(output);
    if !path.is_absolute() {
        return Err(());
    }
    Ok(Some(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_rejects_invalid_paths_and_recognizes_cancellation() {
        assert_eq!(parse_response(b""), Ok(None));
        #[cfg(not(target_os = "windows"))]
        assert_eq!(parse_response(b"\n"), Ok(None));
        assert!(parse_response(b"relative").is_err());
        assert!(parse_response(b"/first\n/second\n").is_err());
        assert!(parse_response(b"/bad\0path").is_err());
        assert!(parse_response(b"\xff").is_err());
        assert!(parse_response(&vec![b'a'; 32 * 1024 + 1]).is_err());
        #[cfg(not(target_os = "windows"))]
        assert_eq!(
            parse_response(b"/tmp/a b\n"),
            Ok(Some(PathBuf::from("/tmp/a b")))
        );
        #[cfg(target_os = "windows")]
        assert_eq!(
            parse_response(b"C:\\a b"),
            Ok(Some(PathBuf::from("C:\\a b")))
        );
    }
}
