fn main() -> std::process::ExitCode {
    rust_processor::update_cli::run_cli(std::env::args().skip(1))
}
