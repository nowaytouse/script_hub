fn main() -> std::process::ExitCode {
    if rust_processor::app::print_version("update") {
        return std::process::ExitCode::SUCCESS;
    }
    rust_processor::update_cli::run_cli(std::env::args().skip(1))
}
