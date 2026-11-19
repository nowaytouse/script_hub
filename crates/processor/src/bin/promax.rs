use std::path::PathBuf;

fn main() {
    if rust_processor::app::print_version("promax") {
        return;
    }
    let mut root = None;
    let mut execute = false;
    let mut refresh_index = false;
    let mut heidai_output = None;
    let mut heidai_bundle = None;
    let mut singbox = None;
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
            "--export-heidai-full" => match arguments.next() {
                Some(value) if !value.is_empty() && !value.starts_with('-') => {
                    heidai_output = Some(PathBuf::from(value));
                }
                _ => {
                    eprintln!("Missing file for --export-heidai-full <file.sgmodule>");
                    std::process::exit(2);
                }
            },
            "--export-heidai-bundle" => match arguments.next() {
                Some(value) if !value.is_empty() && !value.starts_with('-') => {
                    heidai_bundle = Some(PathBuf::from(value));
                }
                _ => {
                    eprintln!("Missing directory for --export-heidai-bundle <new-directory>");
                    std::process::exit(2);
                }
            },
            "--singbox" => match arguments.next() {
                Some(value) if !value.is_empty() && !value.starts_with('-') => {
                    singbox = Some(PathBuf::from(value));
                }
                _ => {
                    eprintln!("Missing executable for --singbox <executable>");
                    std::process::exit(2);
                }
            },
            "--help" | "-h" => {
                println!(
                    "Usage: promax --root <repository> [--execute | --refresh-index | --export-heidai-full <file.sgmodule> | --export-heidai-bundle <new-directory> [--singbox <executable>]]"
                );
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
    if let Some(directory) = heidai_bundle {
        if execute || refresh_index || heidai_output.is_some() {
            eprintln!("--export-heidai-bundle is a separate local export mode");
            std::process::exit(2);
        }
        match rust_processor::pipeline::heidai::export_heidai_bundle(
            &root,
            &directory,
            singbox.as_deref(),
        ) {
            Ok(report) => println!("{report}"),
            Err(error) => {
                eprintln!("Unable to export complete 217heidai bundle: {error}");
                std::process::exit(1);
            }
        }
        return;
    }
    if singbox.is_some() {
        eprintln!("--singbox requires --export-heidai-bundle");
        std::process::exit(2);
    }
    if let Some(output) = heidai_output {
        if execute || refresh_index {
            eprintln!("--export-heidai-full is a separate local export mode");
            std::process::exit(2);
        }
        match rust_processor::pipeline::heidai::export_heidai_full(&root, &output) {
            Ok(report) => println!("{report}"),
            Err(error) => {
                eprintln!("Unable to export complete 217heidai source: {error}");
                std::process::exit(1);
            }
        }
        return;
    }
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
