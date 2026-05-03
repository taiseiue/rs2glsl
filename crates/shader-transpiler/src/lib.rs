mod codegen_glsl;

pub fn transpile_to_glsl(source: &str) -> Result<String, Box<dyn std::error::Error>> {
    let file = syn::parse_file(source)?;
    codegen_glsl::generate(&file).map_err(Into::into)
}
