use crate::errors::TranspileError;
use crate::types::GlslType;
use super::{Tail, TypeEnv};
use super::expr::{extract_ident, generate_expr};

pub(super) fn generate_block(block: &syn::Block, env: &mut TypeEnv, tail: Tail<'_>) -> Result<String, TranspileError> {
    let mut out = String::new();
    let stmts = &block.stmts;

    for (i, stmt) in stmts.iter().enumerate() {
        let is_last = i == stmts.len() - 1;

        match stmt {
            syn::Stmt::Local(local) => {
                let name = extract_ident(&local.pat)?;
                let init_expr = local.init
                    .as_ref()
                    .ok_or(TranspileError::UnsupportedSyntax("let without initializer"))?
                    .expr
                    .as_ref();

                if let syn::Expr::If(if_expr) = init_expr {
                    let ty = infer_block_tail_type(&if_expr.then_branch, env)?;
                    env.insert(name.clone(), ty.clone());
                    out.push_str(&format!("{} {name};\n", ty.to_glsl()));
                    out.push_str(&generate_if(if_expr, Tail::Assign(&name.clone()), env)?);
                } else {
                    let (expr_str, ty) = generate_expr(init_expr, env)?;
                    env.insert(name.clone(), ty.clone());
                    out.push_str(&format!("{} {name} = {expr_str};\n", ty.to_glsl()));
                }
            }

            syn::Stmt::Expr(expr, semi) => {
                if let syn::Expr::If(if_expr) = expr {
                    out.push_str(&generate_if(if_expr, Tail::Discard, env)?);
                } else if is_last && semi.is_none() {
                    let (expr_str, _) = generate_expr(expr, env)?;
                    let line = match tail {
                        Tail::Return       => format!("return {expr_str};\n"),
                        Tail::Assign(name) => format!("{name} = {expr_str};\n"),
                        Tail::Discard      => format!("{expr_str};\n"),
                    };
                    out.push_str(&line);
                } else {
                    let (expr_str, _) = generate_expr(expr, env)?;
                    out.push_str(&format!("{expr_str};\n"));
                }
            }

            _ => return Err(TranspileError::UnsupportedSyntax("statement kind")),
        }
    }

    Ok(out)
}

pub(super) fn infer_block_tail_type(block: &syn::Block, env: &TypeEnv) -> Result<GlslType, TranspileError> {
    let tail = block.stmts.iter().last()
        .ok_or(TranspileError::UnsupportedSyntax("empty if branch"))?;
    match tail {
        syn::Stmt::Expr(expr, None) => Ok(generate_expr(expr, env)?.1),
        _ => Err(TranspileError::UnsupportedSyntax("if expression branch must end with an expression")),
    }
}

pub(super) fn generate_if(if_expr: &syn::ExprIf, tail: Tail<'_>, env: &mut TypeEnv) -> Result<String, TranspileError> {
    let (cond_str, _) = generate_expr(&if_expr.cond, env)?;
    let then_body = generate_block(&if_expr.then_branch, env, tail)?;

    let else_str = match &if_expr.else_branch {
        None => String::new(),
        Some((_, else_expr)) => match else_expr.as_ref() {
            syn::Expr::Block(b) => {
                let body = generate_block(&b.block, env, tail)?;
                format!(" else {{\n{body}}}")
            }
            syn::Expr::If(nested) => {
                format!(" else {}", generate_if(nested, tail, env)?)
            }
            _ => return Err(TranspileError::UnsupportedSyntax("else branch form")),
        },
    };

    Ok(format!("if ({cond_str}) {{\n{then_body}}}{else_str}\n"))
}
