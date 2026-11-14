//! `cargo kabl`: create, build and validate kabl code modules. GPL-3.0-or-later (the SDK and the
//! template it copies are 0BSD). Preview: the commands and flags may change before kabl 1.0.

mod build;
mod new;
mod pack;
mod validate;
mod wasm;

use std::process::ExitCode;

const USAGE: &str = "cargo kabl <command>

  new <githubhandle.module> [--audio|--control] [--into DIR] [--sdk-path DIR]
        create a module crate from the template
  build [--manifest-path FILE] [-p CRATE] [--locked] [--allow-reserved]
        reproducible wasm build, adds kabl.notices, validates
  validate <file.wasm> [--allow-reserved]
        check a built module the way the host would, plus a smoke render";

pub fn print_report(r: &validate::Report) {
    for n in &r.notes {
        println!("  {n}");
    }
    for w in &r.warnings {
        println!("warning: {w}");
    }
    for e in &r.errors {
        println!("error: {e}");
    }
}

fn run(args: &[String]) -> Result<(), String> {
    // Cargo runs `cargo-kabl kabl <args>`; a direct call has no leading `kabl`.
    let args = args.strip_prefix(&["kabl".to_owned()]).unwrap_or(args);
    let Some((cmd, rest)) = args.split_first() else {
        return Err(USAGE.into());
    };
    match cmd.as_str() {
        "new" => {
            let dir = new::run(&new::parse(rest)?)?;
            println!(
                "created {}; next: cd {} && cargo kabl build",
                dir.display(),
                dir.display()
            );
            Ok(())
        }
        "build" => build::run(&build::parse(rest)?).map(drop),
        "validate" => {
            let allow_reserved = rest.iter().any(|a| a == "--allow-reserved");
            let files: Vec<_> = rest.iter().filter(|a| *a != "--allow-reserved").collect();
            let [file] = files[..] else {
                return Err("usage: cargo kabl validate <file.wasm> [--allow-reserved]".into());
            };
            let bytes = std::fs::read(file).map_err(|e| format!("{file}: {e}"))?;
            let report = validate::validate(&bytes, validate::Options { allow_reserved });
            print_report(&report);
            if report.ok() {
                println!("ok");
                Ok(())
            } else {
                Err("validation failed".into())
            }
        }
        "-h" | "--help" | "help" => {
            println!("{USAGE}");
            Ok(())
        }
        other => Err(format!("unknown command `{other}`\n\n{USAGE}")),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
