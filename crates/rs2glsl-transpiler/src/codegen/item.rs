use super::expr::{extract_ident, generate_expr};
use super::stmt::generate_block;
use super::structs::StructRegistry;
use super::ty::{parse_param_type, parse_type};
use super::{FuncRegistry, Tail, TypeAliasMap, TypeEnv};
use crate::errors::TranspileError;
use crate::types::GlslType;

struct FunctionParam {
    name: String,
    ty: GlslType,
    is_out: bool,
}

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
    Ok((
        format!("const {} = {expr_str};\n", ty.render_decl(&name)),
        ty,
    ))
}

pub(super) fn generate_function_declaration(
    func: &syn::ItemFn,
    glsl_name: &str,
    registry: &StructRegistry,
    aliases: &TypeAliasMap,
) -> Result<String, TranspileError> {
    let (params, ret) = build_function_signature(func, registry, aliases)?;
    let args = format_function_params(&params);
    Ok(format!("{ret} {glsl_name}({args});"))
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
    let (params, ret) = build_function_signature(func, registry, aliases)?;
    for param in &params {
        env.insert(param.name.clone(), param.ty.clone());
    }
    let args = format_function_params(&params);
    let tail = match &func.sig.output {
        syn::ReturnType::Type(_, _) => Tail::Return,
        syn::ReturnType::Default => Tail::Discard,
    };

    let body = generate_block(
        &func.block,
        &mut env,
        registry,
        func_registry,
        aliases,
        tail,
    )?;

    Ok(format!("{ret} {glsl_name}({args}) {{\n{body}}}"))
}

fn build_function_signature(
    func: &syn::ItemFn,
    registry: &StructRegistry,
    aliases: &TypeAliasMap,
) -> Result<(Vec<FunctionParam>, String), TranspileError> {
    let params = func
        .sig
        .inputs
        .iter()
        .map(|arg| -> Result<FunctionParam, TranspileError> {
            match arg {
                syn::FnArg::Typed(pat) => {
                    let name = extract_ident(&pat.pat)?;
                    let (ty, is_out) = parse_param_type(&pat.ty, registry, aliases)?;
                    Ok(FunctionParam { name, ty, is_out })
                }
                _ => Err(TranspileError::UnsupportedSyntax("self argument")),
            }
        })
        .collect::<Result<Vec<_>, _>>()?;

    let ret = match &func.sig.output {
        syn::ReturnType::Type(_, ty) => parse_type(ty, registry, aliases)?.render_return_type(),
        syn::ReturnType::Default => "void".to_string(),
    };

    Ok((params, ret))
}

fn format_function_params(params: &[FunctionParam]) -> String {
    params
        .iter()
        .map(|param| {
            let qualifier = if param.is_out { "out " } else { "" };
            format!("{qualifier}{}", param.ty.render_decl(&param.name))
        })
        .collect::<Vec<_>>()
        .join(", ")
}
