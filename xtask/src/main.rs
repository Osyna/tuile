//! Development tasks: `cargo xtask <layers|shot|spinners>`.
//!
//! Run from the repository root. These replace the shell and Python scripts the project
//! used to carry, so the only toolchain a contributor needs is Rust.

mod layers;
mod shot;
mod spinners;

use std::process::ExitCode;

const USAGE: &str = "\
cargo xtask <command>

  layers                 assert the internal module layering (no foundation -> widgets, no cycles)
  spinners               regenerate tuile/src/widgets/spinner/spinners.rs from xtask/spinners.json
  shot [opts] -- CMD     screenshot a TUI headlessly through tmux

shot options:
  -s, --size COLSxROWS   terminal size (default 130x42)
  -k, --keys \"Tab Enter\"  tmux key names sent before the capture
  -m, --mouse \"click X Y\" mouse step, repeatable: click down up drag move wheelup wheeldown
  -o, --out PATH         PNG path (default /tmp/shot.png)
      --text             print the plain-text capture instead of writing a PNG
      --startup SECS     wait after launch (default 0.8)
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("layers") => layers::run(),
        Some("spinners") => spinners::run(),
        Some("shot") => shot::run(&args[1..]),
        Some("-h") | Some("--help") | Some("help") | None => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Some(other) => Err(format!("unknown command `{other}`\n\n{USAGE}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

/// The repository root, found by walking up from this crate.
pub fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask/ always has a parent")
        .to_path_buf()
}
