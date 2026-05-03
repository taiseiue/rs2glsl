use super::expr::{extract_ident, generate_expr};
use super::stmt::generate_block;
use super::structs::StructRegistry;
use super::ty::{parse_param_type, parse_type};
use super::{FuncRegistry, Tail, TypeAliasMap, TypeEnv};
use crate::errors::TranspileError;
use crate::types::GlslType;

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

    let args = func
        .sig
        .inputs
        .iter()
        .map(|arg| -> Result<String, TranspileError> {
            match arg {
                syn::FnArg::Typed(pat) => {
                    let param_name = extract_ident(&pat.pat)?;
                    let (ty, is_out) = parse_param_type(&pat.ty, registry, aliases)?;
                    env.insert(param_name.clone(), ty.clone());
                    let qualifier = if is_out { "out " } else { "" };
                    Ok(format!("{qualifier}{} {param_name}", ty.to_glsl()))
                }
                _ => Err(TranspileError::UnsupportedSyntax("self argument")),
            }
        })
        .collect::<Result<Vec<_>, _>>()?
        .join(", ");

    let (ret, tail) = match &func.sig.output {
        syn::ReturnType::Type(_, ty) => (
            parse_type(ty, registry, aliases)?.to_glsl().to_string(),
            Tail::Return,
        ),
        syn::ReturnType::Default => ("void".to_string(), Tail::Discard),
    };

    let body = generate_block(&func.block, &mut env, registry, func_registry, tail)?;

    Ok(format!("{ret} {glsl_name}({args}) {{\n{body}}}"))
}
