pub mod update_cli;
pub mod validate_cli;

/// Prints the Cargo package version for a standalone version request.
pub fn print_version(binary: &str) -> bool {
    if std::env::args().len() == 2
        && matches!(std::env::args().nth(1).as_deref(), Some("--version" | "-V"))
    {
        println!("{binary} {}", env!("CARGO_PKG_VERSION"));
        true
    } else {
        false
    }
}
