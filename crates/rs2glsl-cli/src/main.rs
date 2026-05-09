use std::{env, process};

fn main() {
    let paths = env::args().skip(1).collect::<Vec<_>>();
    if paths.is_empty() {
        eprintln!("usage: cargo-rs2glsl <path> [path ...]");
        process::exit(1);
    }

    let source = rs2glsl_resolver::read_sources(&paths).unwrap_or_else(|e| {
        eprintln!("{e}");
        process::exit(1);
    });

    match rs2glsl_transpiler::transpile_to_glsl(&source) {
        Ok(glsl) => print!("{glsl}"),
        Err(e) => {
            if let Some(location) = e.location() {
                eprintln!(
                    "Error[{}] at line {}, column {}: {e}",
                    e.code(),
                    location.line,
                    location.column
                );
            } else {
                eprintln!("Error[{}]: {e}", e.code());
            }
            process::exit(1);
        }
    }
}
