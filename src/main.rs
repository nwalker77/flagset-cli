use std::env;
use std::fs;
use std::process::ExitCode;

fn usage() -> ExitCode {
    eprintln!("usage: flagset <config-file> list");
    eprintln!("       flagset <config-file> check <flag> <key>");
    ExitCode::FAILURE
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() < 2 {
        return usage();
    }

    let config_path = &args[0];
    let command = args[1].as_str();

    let contents = match fs::read_to_string(config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: could not read {config_path}: {e}");
            return ExitCode::FAILURE;
        }
    };

    let flagset = match flagset_cli::parse(&contents) {
        Ok(fs) => fs,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    match command {
        "list" => {
            let mut names: Vec<&str> = flagset.names().collect();
            names.sort_unstable();
            for name in names {
                println!("{name} {}", flagset.describe(name).unwrap());
            }
            ExitCode::SUCCESS
        }
        "check" => {
            if args.len() != 4 {
                eprintln!("usage: flagset <config-file> check <flag> <key>");
                return ExitCode::FAILURE;
            }
            let flag = &args[2];
            let key = &args[3];
            match flagset.is_enabled(flag, key) {
                Ok(true) => {
                    println!("on");
                    ExitCode::SUCCESS
                }
                Ok(false) => {
                    println!("off");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => usage(),
    }
}
