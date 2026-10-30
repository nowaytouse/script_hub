#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn main() -> std::process::ExitCode {
    rust_processor::desktop::app::run(std::env::args().skip(1))
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn main() -> std::process::ExitCode {
    eprintln!("Linux desktop GUI development is frozen. Use the Rust CLI on this platform.");
    std::process::ExitCode::from(2)
}
