use std::{env, fs};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args().nth(1).expect("usage: shader-cli <file.rs>");
    let source = fs::read_to_string(path)?;
    let glsl = shader_transpiler::transpile_to_glsl(&source)?;
    println!("{glsl}");
    Ok(())
}
