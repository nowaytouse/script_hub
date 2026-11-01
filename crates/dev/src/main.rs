fn main() -> std::process::ExitCode {
    match script_hub_dev::run(std::env::args().skip(1)) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("[ERROR] {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
