#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

fn main() {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    let result = if arguments.is_empty() {
        locate_workspace_root().map_or_else(
            |message| Err(vec![arch_check::Violation { line: 1, message }]),
            |root| arch_check::check_workspace_r0_04(&root),
        )
    } else {
        run_fixture_mode(&arguments)
    };

    match result {
        Ok(()) => println!("architecture checks passed"),
        Err(violations) => {
            for violation in violations {
                eprintln!("{}", violation.message);
            }
            std::process::exit(1);
        }
    }
}

fn run_fixture_mode(arguments: &[std::ffi::OsString]) -> Result<(), Vec<arch_check::Violation>> {
    if arguments.len() != 4 || arguments[0] != "--metadata-fixture" || arguments[2] != "--config" {
        return Err(vec![arch_check::Violation {
            line: 1,
            message: "usage: arch-check [--metadata-fixture PATH --config PATH]".to_owned(),
        }]);
    }
    arch_check::check_fixture_files(Path::new(&arguments[1]), Path::new(&arguments[3]))
}

fn locate_workspace_root() -> Result<PathBuf, String> {
    let mut directory = std::env::current_dir()
        .map_err(|error| format!("cannot determine current directory: {error}"))?;
    loop {
        if directory.join("tools/arch-check/arch.toml").is_file()
            && directory.join("Cargo.toml").is_file()
        {
            return Ok(directory);
        }
        if !directory.pop() {
            return Err(
                "cannot locate workspace root containing tools/arch-check/arch.toml".to_owned(),
            );
        }
    }
}
