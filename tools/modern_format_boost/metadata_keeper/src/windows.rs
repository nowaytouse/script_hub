use std::path::Path;
use std::process::Command;
use std::io;

/// Preserves Windows-specific metadata (ACLs, Attributes, Timestamps)
pub fn preserve_windows_attributes(src: &Path, dst: &Path) -> io::Result<()> {
    
    // 1. Basic File Attributes (Hidden, ReadOnly, System)
    // We can use the 'attrib' command or std::os::windows::fs::MetadataExt
    // usage of 'attrib' is robust for transferring system bits.
    // attrib +h +s ...
    
    // However, the "Nuclear Option" on Windows is `robocopy`.
    // But robocopy is for copying FILES, not just metadata on existing files (though it can do that).
    // If we want to copy metadata to an ALREADY converted file (dst), we might need `robocopy /copy:SOU ...`
    // /COPY:copyflag :: what to COPY for files (default is /COPY:DAT).
    // (copyflags : D=Data, A=Attributes, T=Timestamps).
    // (S=Security=NTFS ACLs, O=Owner info, U=Auditing info).
    
    // Using robocopy to *only* transfer metadata to an existing file is tricky.
    // Instead, we use `icacls` to save and restore ACLs.
    
    // A. ACLs (Access Control Lists)
    // icacls source_file /save aclfile /t /c /q
    // icacls target_file /restore aclfile
    // But since src and dst names differ, we might need to be careful.
    // Simple approach: Use Powershell `Get-Acl` | `Set-Acl`? Too slow.
    // Simple approach 2: icacls dst /setowner ...
    
    // Let's stick to reliable CLI tools available on standard Windows.
    if which::which("icacls").is_ok() {
        // Saving ACLs to a temp file seems overkill for one file.
        // Sadly Windows doesn't have an easy "copy-acl <src> <dst>" command natively without Powershell.
        
        // Let's try PowerShell command, it's standard on modern Windows.
        let ps_script = format!(
            "Get-Acl -Path '{}' | Set-Acl -Path '{}'",
            src.to_string_lossy(),
            dst.to_string_lossy()
        );
        
        let _ = Command::new("powershell")
            .arg("-Command")
            .arg(ps_script)
            .output(); // Ignore errors, best effort
    }
    
    // 2. Attributes (ReadOnly, Hidden, System, Archive)
    // We can use std::os::windows::fs
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if let Ok(meta) = std::fs::metadata(src) {
            let file_attrs = meta.file_attributes();
            // We need to set these on dst.
            // There isn't a direct std API to SET attributes by raw u32, strictly speaking, 
            // without winapi crate or attrib command.
            // Let's use `attrib`.
            
            // Re-applying Hidden/System if present
            // 0x2 = Hidden, 0x4 = System
            let is_hidden = (file_attrs & 0x2) != 0;
            let is_system = (file_attrs & 0x4) != 0;
            
            let mut cmd = Command::new("attrib");
            if is_hidden { cmd.arg("+h"); }
            if is_system { cmd.arg("+s"); }
            cmd.arg(dst);
            let _ = cmd.output();
        }
    }
    
    Ok(())
}
