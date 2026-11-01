use std::path::PathBuf;

fn main() {
    if rust_processor::app::print_version("promax") {
        return;
    }
    let mut root = None;
    let mut execute = false;
    let mut refresh_index = false;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--root" => match arguments.next() {
                Some(value) if !value.is_empty() && !value.starts_with('-') => {
                    root = Some(PathBuf::from(value));
                }
                _ => {
                    eprintln!("Missing value for --root <repository>");
                    std::process::exit(2);
                }
            },
            "--execute" => execute = true,
            "--refresh-index" => refresh_index = true,
            "--help" | "-h" => {
                println!("Usage: promax --root <repository> [--execute | --refresh-index]");
                return;
            }
            unknown => {
                eprintln!("Unknown argument: {unknown}");
                std::process::exit(2);
            }
        }
    }

    let Some(root) = root else {
        eprintln!("Missing required --root <repository> argument");
        std::process::exit(2);
    };
    if refresh_index {
        if execute {
            eprintln!("--refresh-index cannot be combined with --execute");
            std::process::exit(2);
        }
        if let Err(error) = rust_processor::promax::refresh_published_index(&root) {
            eprintln!("Unable to refresh published index: {error}");
            std::process::exit(1);
        }
        return;
    }
    let root = root.to_string_lossy();
    if !rust_processor::promax::run_adblock_manager(&root, execute) {
        std::process::exit(1);
    }
}
