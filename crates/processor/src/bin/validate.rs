fn main() -> std::process::ExitCode {
    if rust_processor::app::print_version("validate") {
        return std::process::ExitCode::SUCCESS;
    }
    rust_processor::validate_cli::run_cli(std::env::args().skip(1))
}
