use std::path::PathBuf;

fn main() {
    let mut root = None;
    let mut execute = false;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--root" => root = arguments.next().map(PathBuf::from),
            "--execute" => execute = true,
            "--help" | "-h" => {
                println!("Usage: promax --root <repository> [--execute]");
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
    let root = root.to_string_lossy();
    if !rust_processor::promax::run_adblock_manager(&root, execute) {
        std::process::exit(1);
    }
}
