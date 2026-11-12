use std::process::ExitCode;

const USAGE: &str = "Usage: routing_template <surge|shadowrocket|flclash|mihomo|sing-box> <direct-policy> <proxy-policy>\nMerge the emitted routing entries before broad rules; keep the existing final rule.";

fn main() -> ExitCode {
    if rust_processor::app::print_version("routing_template") {
        return ExitCode::SUCCESS;
    }
    let args: Vec<_> = std::env::args().skip(1).collect();
    if matches!(args.as_slice(), [arg] if matches!(arg.as_str(), "--help" | "-h")) {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let [client, direct, proxy] = args.as_slice() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    match rust_processor::pipeline::routing_template::render(client, direct, proxy) {
        Ok(fragment) => {
            println!("{}", fragment.trim_end());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}
