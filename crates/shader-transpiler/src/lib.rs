mod codegen;
mod errors;
mod types;

pub use errors::TranspileError;

pub fn transpile_to_glsl(source: &str) -> Result<String, TranspileError> {
    let file = syn::parse_file(source)
        .map_err(|_| TranspileError::UnsupportedSyntax("Rust syntax error"))?;
    codegen::generate(&file)
}
