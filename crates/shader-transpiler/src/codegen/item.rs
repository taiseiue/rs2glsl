use crate::errors::TranspileError;
use crate::types::GlslType;
use super::{Tail, TypeEnv, TypeAliasMap, FuncRegistry};
use super::structs::StructRegistry;
use super::expr::{extract_ident, generate_expr};
use super::stmt::generate_block;
use super::ty::parse_type;

pub(super) fn generate_const(
    item: &syn::ItemConst,
    env: &TypeEnv,
    registry: &StructRegistry,
    aliases: &TypeAliasMap,
    func_registry: &FuncRegistry,
) -> Result<(String, GlslType), TranspileError> {
    let name = item.ident.to_string();
    let ty = parse_type(&item.ty, registry, aliases)?;
    let (expr_str, _) = generate_expr(&item.expr, env, registry, func_registry)?;
    Ok((format!("const {} {name} = {expr_str};\n", ty.to_glsl()), ty))
}

pub(super) fn generate_function(
    func: &syn::ItemFn,
    glsl_name: &str,
    global_env: &TypeEnv,
    registry: &StructRegistry,
    aliases: &TypeAliasMap,
    func_registry: &FuncRegistry,
) -> Result<String, TranspileError> {
    let mut env = global_env.clone();

    let args = func.sig.inputs.iter().map(|arg| -> Result<String, TranspileError> {
        match arg {
            syn::FnArg::Typed(pat) => {
                let param_name = extract_ident(&pat.pat)?;
                let ty = parse_type(&pat.ty, registry, aliases)?;
                env.insert(param_name.clone(), ty.clone());
                Ok(format!("{} {param_name}", ty.to_glsl()))
            }
            _ => Err(TranspileError::UnsupportedSyntax("self argument")),
        }
    }).collect::<Result<Vec<_>, _>>()?.join(", ");

    let ret = match &func.sig.output {
        syn::ReturnType::Type(_, ty) => parse_type(ty, registry, aliases)?.to_glsl().to_string(),
        syn::ReturnType::Default => "void".to_string(),
    };

    let body = generate_block(&func.block, &mut env, registry, func_registry, Tail::Return)?;

    Ok(format!("{ret} {glsl_name}({args}) {{\n{body}\n}}"))
}
