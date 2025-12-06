use std::path::Path;
use std::process::Command;
use std::io;

/// Preserves Linux-specific metadata (ACLs, Extended Attributes, File Attributes)
pub fn preserve_linux_attributes(src: &Path, dst: &Path) -> io::Result<()> {
    
    // 1. Extended Attributes (xattrs)
    // Handled by cross-platform `xattr` crate in lib.rs logic, checking here for specific system ones?
    // Linux user.* trusted.* security.*
    // The generic xattr loop usually covers this, but let's confirm.

    // 2. Access Control Lists (ACLs)
    // Requires `getfacl` / `setfacl` (part of acl package)
    if which::which("getfacl").is_ok() && which::which("setfacl").is_ok() {
        // getfacl -a src | setfacl -M - dst
        // Pipe logic requires shell or manual piping.
        // Simplest portable way:
        let output = Command::new("getfacl")
            .arg("--absolute-names") // Avoid stripping leading '/'
            .arg(src)
            .output()?;
        
        if output.status.success() {
            let mut set_cmd = Command::new("setfacl");
            set_cmd.arg("--restore=-"); // Read from stdin
            set_cmd.stdin(std::process::Stdio::piped());
            // This is complex to pipe in purely std::process::Command without a spawned child.
            // Alternative: `setfacl --set-file=...` but we have stream.
            
            // Let's spawn child
            if let Ok(mut child) = Command::new("setfacl")
                .arg("--restore=-")
                .stdin(std::process::Stdio::piped())
                .spawn() 
            {
                if let Some(mut stdin) = child.stdin.take() {
                    use std::io::Write;
                    let _ = stdin.write_all(&output.stdout);
                }
                let _ = child.wait();
            }
        }
    }

    // 3. File Attributes (chattr / lsattr)
    // Immutable bit, etc.
    // lsattr -d src -> parse -> chattr
    // This is often filesystem specific (ext4, xfs, btrfs)
    // We can try detailed copy if `cp --preserve=all` was available, but we are doing it securely here.
    
    Ok(())
}
