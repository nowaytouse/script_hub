fn main() -> std::process::ExitCode {
    rust_processor::validate_cli::run_cli(std::env::args().skip(1))
}
