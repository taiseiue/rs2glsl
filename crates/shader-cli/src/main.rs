use std::{env, fs, process};

fn main() {
    let path = env::args().nth(1).expect("usage: shader-cli <file.rs>");
    let source = fs::read_to_string(&path).unwrap_or_else(|e| {
        eprintln!("File read error: {e}");
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
