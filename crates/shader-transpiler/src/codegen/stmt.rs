use super::expr::{extract_ident, generate_expr};
use super::structs::StructRegistry;
use super::{FuncRegistry, Tail, TypeEnv};
use crate::errors::TranspileError;
use crate::types::GlslType;

pub(super) fn generate_block(
    block: &syn::Block,
    env: &mut TypeEnv,
    registry: &StructRegistry,
    func_registry: &FuncRegistry,
    tail: Tail<'_>,
) -> Result<String, TranspileError> {
    let mut out = String::new();
    let stmts = &block.stmts;

    for (i, stmt) in stmts.iter().enumerate() {
        let is_last = i == stmts.len() - 1;

        match stmt {
            syn::Stmt::Local(local) => {
                let name = extract_ident(&local.pat)?;
                let init_expr = local
                    .init
                    .as_ref()
                    .ok_or(TranspileError::UnsupportedSyntax("let without initializer"))?
                    .expr
                    .as_ref();

                if let syn::Expr::If(if_expr) = init_expr {
                    let ty =
                        infer_block_tail_type(&if_expr.then_branch, env, registry, func_registry)?;
                    env.insert(name.clone(), ty.clone());
                    out.push_str(&format!("{} {name};\n", ty.to_glsl()));
                    out.push_str(&generate_if(
                        if_expr,
                        Tail::Assign(&name.clone()),
                        env,
                        registry,
                        func_registry,
                    )?);
                } else {
                    let (expr_str, ty) = generate_expr(init_expr, env, registry, func_registry)?;
                    env.insert(name.clone(), ty.clone());
                    out.push_str(&format!("{} {name} = {expr_str};\n", ty.to_glsl()));
                }
            }

            syn::Stmt::Expr(expr, semi) => {
                if let syn::Expr::If(if_expr) = expr {
                    out.push_str(&generate_if(
                        if_expr,
                        Tail::Discard,
                        env,
                        registry,
                        func_registry,
                    )?);
                } else if let syn::Expr::ForLoop(for_loop) = expr {
                    out.push_str(&generate_for(for_loop, env, registry, func_registry)?);
                } else if is_last && semi.is_none() {
                    let (expr_str, _) = generate_expr(expr, env, registry, func_registry)?;
                    let line = match tail {
                        Tail::Return => format!("return {expr_str};\n"),
                        Tail::Assign(name) => format!("{name} = {expr_str};\n"),
                        Tail::Discard => format!("{expr_str};\n"),
                    };
                    out.push_str(&line);
                } else {
                    let (expr_str, _) = generate_expr(expr, env, registry, func_registry)?;
                    out.push_str(&format!("{expr_str};\n"));
                }
            }

            _ => return Err(TranspileError::UnsupportedSyntax("statement kind")),
        }
    }

    Ok(out)
}

fn expect_int(ty: &GlslType, context: &'static str) -> Result<(), TranspileError> {
    if matches!(ty.primitive(), GlslType::Int) {
        Ok(())
    } else {
        Err(TranspileError::UnsupportedSyntax(context))
    }
}

pub(super) fn generate_for(
    for_loop: &syn::ExprForLoop,
    env: &TypeEnv,
    registry: &StructRegistry,
    func_registry: &FuncRegistry,
) -> Result<String, TranspileError> {
    let loop_var = extract_ident(&for_loop.pat)?;
    let range = match for_loop.expr.as_ref() {
        syn::Expr::Range(range) => range,
        _ => {
            return Err(TranspileError::UnsupportedSyntax(
                "for loop iterable must be a range",
            ));
        }
    };

    let start_expr = range
        .start
        .as_ref()
        .ok_or(TranspileError::UnsupportedSyntax(
            "for loop range must have a start bound",
        ))?;
    let end_expr = range.end.as_ref().ok_or(TranspileError::UnsupportedSyntax(
        "for loop range must have an end bound",
    ))?;

    let (start_str, start_ty) = generate_expr(start_expr, env, registry, func_registry)?;
    let (end_str, end_ty) = generate_expr(end_expr, env, registry, func_registry)?;
    expect_int(&start_ty, "for loop start bound must be an integer")?;
    expect_int(&end_ty, "for loop end bound must be an integer")?;

    let cond_op = match range.limits {
        syn::RangeLimits::HalfOpen(_) => "<",
        syn::RangeLimits::Closed(_) => "<=",
    };

    let mut loop_env = env.clone();
    loop_env.insert(loop_var.clone(), GlslType::Int);
    let body = generate_block(
        &for_loop.body,
        &mut loop_env,
        registry,
        func_registry,
        Tail::Discard,
    )?;

    Ok(format!(
        "for (int {loop_var} = {start_str}; {loop_var} {cond_op} {end_str}; {loop_var}++) {{\n{body}}}\n"
    ))
}

pub(super) fn infer_block_tail_type(
    block: &syn::Block,
    env: &TypeEnv,
    registry: &StructRegistry,
    func_registry: &FuncRegistry,
) -> Result<GlslType, TranspileError> {
    let tail = block
        .stmts
        .iter()
        .last()
        .ok_or(TranspileError::UnsupportedSyntax("empty if branch"))?;
    match tail {
        syn::Stmt::Expr(expr, None) => Ok(generate_expr(expr, env, registry, func_registry)?.1),
        _ => Err(TranspileError::UnsupportedSyntax(
            "if expression branch must end with an expression",
        )),
    }
}

pub(super) fn generate_if(
    if_expr: &syn::ExprIf,
    tail: Tail<'_>,
    env: &mut TypeEnv,
    registry: &StructRegistry,
    func_registry: &FuncRegistry,
) -> Result<String, TranspileError> {
    let (cond_str, _) = generate_expr(&if_expr.cond, env, registry, func_registry)?;
    let then_body = generate_block(&if_expr.then_branch, env, registry, func_registry, tail)?;

    let else_str = match &if_expr.else_branch {
        None => String::new(),
        Some((_, else_expr)) => match else_expr.as_ref() {
            syn::Expr::Block(b) => {
                let body = generate_block(&b.block, env, registry, func_registry, tail)?;
                format!(" else {{\n{body}}}")
            }
            syn::Expr::If(nested) => {
                format!(
                    " else {}",
                    generate_if(nested, tail, env, registry, func_registry)?
                )
            }
            _ => return Err(TranspileError::UnsupportedSyntax("else branch form")),
        },
    };

    Ok(format!("if ({cond_str}) {{\n{then_body}}}{else_str}\n"))
}
