use std::{env, process};

fn main() {
    let paths = env::args().skip(1).collect::<Vec<_>>();
    if paths.is_empty() {
        eprintln!("usage: shader-cli <file.rs> [file.rs ...]");
        process::exit(1);
    }

    let source = shader_solver::read_sources(&paths).unwrap_or_else(|e| {
        eprintln!("{e}");
        process::exit(1);
    });

    match shader_transpiler::transpile_to_glsl(&source) {
        Ok(glsl) => print!("{glsl}"),
        Err(e) => {
            eprintln!("Error[{}]: {e}", e.code());
            process::exit(1);
        }
    }
}
