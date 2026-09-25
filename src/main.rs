use std::env;
use std::fs;
use std::process::ExitCode;

fn usage() -> ExitCode {
    eprintln!("usage: flagset <config-file> list [--json]");
    eprintln!("       flagset <config-file> check <flag> <key> [--json]");
    eprintln!("       flagset <config-file> validate [--json]");
    ExitCode::FAILURE
}

/// Escapes a string for embedding in the CLI's hand-rolled JSON output.
/// Only needs to cover flag names, keys, and error messages - there's no
/// user-supplied nesting or arbitrary structure to worry about, so this
/// skips pulling in a JSON crate for a handful of `format!` calls.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn print_error(json: bool, message: &str) {
    if json {
        println!("{{\"ok\":false,\"error\":{}}}", json_escape(message));
    } else {
        eprintln!("error: {message}");
    }
}

fn main() -> ExitCode {
    let mut args: Vec<String> = env::args().skip(1).collect();

    // --json can appear anywhere after the config path and command; strip
    // it up front so the positional arguments below don't have to account
    // for it.
    let json = if let Some(pos) = args.iter().position(|a| a == "--json") {
        args.remove(pos);
        true
    } else {
        false
    };

    if args.len() < 2 {
        return usage();
    }

    let config_path = &args[0];
    let command = args[1].as_str();

    let contents = match fs::read_to_string(config_path) {
        Ok(c) => c,
        Err(e) => {
            print_error(json, &format!("could not read {config_path}: {e}"));
            return ExitCode::FAILURE;
        }
    };

    let flagset = match flagset_cli::parse(&contents) {
        Ok(fs) => fs,
        Err(e) => {
            print_error(json, &e.to_string());
            return ExitCode::FAILURE;
        }
    };

    match command {
        "validate" => {
            let count = flagset.names().count();
            if json {
                println!("{{\"ok\":true,\"flags\":{count}}}");
            } else {
                println!("ok: {count} flag(s)");
            }
            ExitCode::SUCCESS
        }
        "list" => {
            let mut names: Vec<&str> = flagset.names().collect();
            names.sort_unstable();
            if json {
                let mut entries = Vec::with_capacity(names.len());
                for name in names {
                    let info = flagset.info(name).unwrap();
                    let expires = match info.expires {
                        Some(d) => json_escape(&d),
                        None => "null".to_string(),
                    };
                    entries.push(format!(
                        "{{\"name\":{},\"enabled\":{},\"rollout\":{},\"expires\":{},\"overrides\":{}}}",
                        json_escape(name),
                        info.enabled,
                        info.rollout,
                        expires,
                        info.overrides
                    ));
                }
                println!("[{}]", entries.join(","));
            } else {
                for name in names {
                    println!("{name} {}", flagset.describe(name).unwrap());
                }
            }
            ExitCode::SUCCESS
        }
        "check" => {
            let rest = &args[2..];
            if rest.len() != 2 {
                print_error(json, "usage: flagset <config-file> check <flag> <key> [--json]");
                return ExitCode::FAILURE;
            }
            let flag = &rest[0];
            let key = &rest[1];
            match flagset.is_enabled(flag, key) {
                Ok(enabled) => {
                    if json {
                        println!(
                            "{{\"flag\":{},\"key\":{},\"enabled\":{}}}",
                            json_escape(flag),
                            json_escape(key),
                            enabled
                        );
                    } else {
                        println!("{}", if enabled { "on" } else { "off" });
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    print_error(json, &e.to_string());
                    ExitCode::FAILURE
                }
            }
        }
        _ => usage(),
    }
}
