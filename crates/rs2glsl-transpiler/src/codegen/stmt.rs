use super::expr::{coerce_expression_to_type, extract_ident, generate_expr, infer_expr_type};
use super::structs::StructRegistry;
use super::ty::{self, parse_type};
use super::{FuncRegistry, Tail, TypeAliasMap, TypeEnv, indent_block};
use crate::errors::TranspileError;
use crate::types::GlslType;

pub(super) fn generate_block(
    block: &syn::Block,
    env: &mut TypeEnv,
    registry: &StructRegistry,
    func_registry: &FuncRegistry,
    aliases: &TypeAliasMap,
    temp_counter: &mut usize,
    tail: Tail<'_>,
) -> Result<String, TranspileError> {
    let mut out = String::new();
    let stmts = &block.stmts;

    for (i, stmt) in stmts.iter().enumerate() {
        let is_last = i == stmts.len() - 1;

        match stmt {
            syn::Stmt::Local(local) => {
                let (name, annotated_ty) = extract_local_binding(&local.pat, registry, aliases)?;
                let init_expr = local
                    .init
                    .as_ref()
                    .ok_or(TranspileError::UnsupportedSyntax("let without initializer"))?
                    .expr
                    .as_ref();

                if let syn::Expr::If(if_expr) = init_expr {
                    let inferred_ty =
                        infer_block_tail_type(&if_expr.then_branch, env, registry, func_registry)?;
                    let ty = annotated_ty.unwrap_or(inferred_ty);
                    env.insert(name.clone(), ty.clone());
                    out.push_str(&format!("{};\n", ty.render_decl(&name)));
                    out.push_str(&generate_if(
                        if_expr,
                        Tail::Assign(&name.clone()),
                        env,
                        registry,
                        func_registry,
                        aliases,
                        temp_counter,
                    )?);
                } else if let syn::Expr::Match(match_expr) = init_expr {
                    let inferred_ty =
                        infer_match_arm_type(match_expr, env, registry, func_registry)?;
                    let ty = annotated_ty.unwrap_or(inferred_ty);
                    env.insert(name.clone(), ty.clone());
                    out.push_str(&format!("{};\n", ty.render_decl(&name)));
                    let assign_name = name.clone();
                    out.push_str(&generate_match(
                        match_expr,
                        Tail::Assign(&assign_name),
                        env,
                        registry,
                        func_registry,
                        aliases,
                        temp_counter,
                    )?);
                } else {
                    let inferred_ty = infer_expr_type(init_expr, env, registry, func_registry)?;
                    let ty = annotated_ty.unwrap_or(inferred_ty);
                    env.insert(name.clone(), ty.clone());
                    if matches!(ty, GlslType::Array(_, _)) {
                        out.push_str(&format!("{};\n", ty.render_decl(&name)));
                        out.push_str(&emit_expr_into_target(
                            &name,
                            &ty,
                            init_expr,
                            env,
                            registry,
                            func_registry,
                            temp_counter,
                        )?);
                    } else {
                        let (expr_str, expr_ty) =
                            generate_expr(init_expr, env, registry, func_registry)?;
                        let expr_str = coerce_expression_to_type(expr_str, &expr_ty, &ty)?;
                        out.push_str(&format!("{} = {expr_str};\n", ty.render_decl(&name)));
                    }
                }
            }

            syn::Stmt::Expr(expr, semi) => {
                if let syn::Expr::If(if_expr) = expr {
                    let if_tail = if is_last && semi.is_none() {
                        tail
                    } else {
                        Tail::Discard
                    };
                    out.push_str(&generate_if(
                        if_expr,
                        if_tail,
                        env,
                        registry,
                        func_registry,
                        aliases,
                        temp_counter,
                    )?);
                } else if let syn::Expr::ForLoop(for_loop) = expr {
                    out.push_str(&generate_for(
                        for_loop,
                        env,
                        registry,
                        func_registry,
                        aliases,
                        temp_counter,
                    )?);
                } else if let syn::Expr::While(while_loop) = expr {
                    out.push_str(&generate_while(
                        while_loop,
                        env,
                        registry,
                        func_registry,
                        aliases,
                        temp_counter,
                    )?);
                } else if let syn::Expr::Break(br) = expr {
                    if br.label.is_some() {
                        return Err(TranspileError::UnsupportedSyntax("labeled break"));
                    }
                    if br.expr.is_some() {
                        return Err(TranspileError::UnsupportedSyntax("break with value"));
                    }
                    out.push_str("break;\n");
                } else if let syn::Expr::Continue(cont) = expr {
                    if cont.label.is_some() {
                        return Err(TranspileError::UnsupportedSyntax("labeled continue"));
                    }
                    out.push_str("continue;\n");
                } else if let syn::Expr::Loop(loop_expr) = expr {
                    let mut loop_env = env.clone();
                    let body = generate_block(
                        &loop_expr.body,
                        &mut loop_env,
                        registry,
                        func_registry,
                        aliases,
                        temp_counter,
                        Tail::Discard,
                    )?;
                    out.push_str(&format!("while (true) {{\n{}}}\n", indent_block(&body)));
                } else if let syn::Expr::Match(match_expr) = expr {
                    let match_tail = if is_last && semi.is_none() {
                        tail
                    } else {
                        Tail::Discard
                    };
                    out.push_str(&generate_match(
                        match_expr,
                        match_tail,
                        env,
                        registry,
                        func_registry,
                        aliases,
                        temp_counter,
                    )?);
                } else if is_last && semi.is_none() {
                    out.push_str(&generate_tail_expr(
                        expr,
                        tail,
                        env,
                        registry,
                        func_registry,
                        temp_counter,
                    )?);
                } else {
                    out.push_str(&generate_statement_expr(
                        expr,
                        env,
                        registry,
                        func_registry,
                        temp_counter,
                    )?);
                }
            }

            _ => return Err(TranspileError::UnsupportedSyntax("statement kind")),
        }
    }

    Ok(out)
}

fn expect_integer(ty: &GlslType, context: &'static str) -> Result<(), TranspileError> {
    if ty.is_integer() {
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
    aliases: &TypeAliasMap,
    temp_counter: &mut usize,
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
    expect_integer(&start_ty, "for loop start bound must be an integer")?;
    expect_integer(&end_ty, "for loop end bound must be an integer")?;
    let loop_ty = if matches!(
        (start_ty.primitive(), end_ty.primitive()),
        (GlslType::Uint, _) | (_, GlslType::Uint)
    ) {
        GlslType::Uint
    } else {
        GlslType::Int
    };
    let start_str = coerce_expression_to_type(start_str, &start_ty, &loop_ty)?;
    let end_str = coerce_expression_to_type(end_str, &end_ty, &loop_ty)?;

    let cond_op = match range.limits {
        syn::RangeLimits::HalfOpen(_) => "<",
        syn::RangeLimits::Closed(_) => "<=",
    };

    let mut loop_env = env.clone();
    loop_env.insert(loop_var.clone(), loop_ty.clone());
    let body = generate_block(
        &for_loop.body,
        &mut loop_env,
        registry,
        func_registry,
        aliases,
        temp_counter,
        Tail::Discard,
    )?;

    Ok(format!(
        "for ({} {loop_var} = {start_str}; {loop_var} {cond_op} {end_str}; {loop_var}++) {{\n{}}}\n",
        loop_ty.to_glsl(),
        indent_block(&body)
    ))
}

pub(super) fn generate_while(
    while_loop: &syn::ExprWhile,
    env: &TypeEnv,
    registry: &StructRegistry,
    func_registry: &FuncRegistry,
    aliases: &TypeAliasMap,
    temp_counter: &mut usize,
) -> Result<String, TranspileError> {
    let (cond_str, _) = generate_expr(&while_loop.cond, env, registry, func_registry)?;
    let mut loop_env = env.clone();
    let body = generate_block(
        &while_loop.body,
        &mut loop_env,
        registry,
        func_registry,
        aliases,
        temp_counter,
        Tail::Discard,
    )?;
    Ok(format!(
        "while ({cond_str}) {{\n{}}}\n",
        indent_block(&body)
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
        syn::Stmt::Expr(expr, None) => infer_expr_type(expr, env, registry, func_registry),
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
    aliases: &TypeAliasMap,
    temp_counter: &mut usize,
) -> Result<String, TranspileError> {
    let (cond_str, _) = generate_expr(&if_expr.cond, env, registry, func_registry)?;
    let then_body = generate_block(
        &if_expr.then_branch,
        env,
        registry,
        func_registry,
        aliases,
        temp_counter,
        tail,
    )?;

    let else_str = match &if_expr.else_branch {
        None => String::new(),
        Some((_, else_expr)) => match else_expr.as_ref() {
            syn::Expr::Block(b) => {
                let body = generate_block(
                    &b.block,
                    env,
                    registry,
                    func_registry,
                    aliases,
                    temp_counter,
                    tail,
                )?;
                format!(" else {{\n{}}}", indent_block(&body))
            }
            syn::Expr::If(nested) => {
                format!(
                    " else {}",
                    generate_if(
                        nested,
                        tail,
                        env,
                        registry,
                        func_registry,
                        aliases,
                        temp_counter,
                    )?
                )
            }
            _ => return Err(TranspileError::UnsupportedSyntax("else branch form")),
        },
    };

    Ok(format!(
        "if ({cond_str}) {{\n{}}}{else_str}\n",
        indent_block(&then_body)
    ))
}

fn extract_local_binding(
    pat: &syn::Pat,
    registry: &StructRegistry,
    aliases: &TypeAliasMap,
) -> Result<(String, Option<GlslType>), TranspileError> {
    match pat {
        syn::Pat::Ident(ident) => Ok((ident.ident.to_string(), None)),
        syn::Pat::Type(typed) => Ok((
            extract_ident(&typed.pat)?,
            Some(parse_type(&typed.ty, registry, aliases)?),
        )),
        _ => Err(TranspileError::UnsupportedSyntax("non-ident pattern")),
    }
}

fn generate_statement_expr(
    expr: &syn::Expr,
    env: &TypeEnv,
    registry: &StructRegistry,
    func_registry: &FuncRegistry,
    temp_counter: &mut usize,
) -> Result<String, TranspileError> {
    match expr {
        syn::Expr::Assign(assign) => {
            let lhs_ty = infer_expr_type(&assign.left, env, registry, func_registry)?;
            if matches!(lhs_ty, GlslType::Array(_, _)) {
                let lhs = generate_assignment_lhs(&assign.left, env, registry, func_registry)?;
                emit_expr_into_target(
                    &lhs,
                    &lhs_ty,
                    &assign.right,
                    env,
                    registry,
                    func_registry,
                    temp_counter,
                )
            } else {
                let lhs = generate_assignment_lhs(&assign.left, env, registry, func_registry)?;
                let (rhs_str, rhs_ty) = generate_expr(&assign.right, env, registry, func_registry)?;
                let rhs_str = coerce_expression_to_type(rhs_str, &rhs_ty, &lhs_ty)?;
                Ok(format!("{lhs} = {rhs_str};\n"))
            }
        }
        syn::Expr::Binary(bin) => {
            let lhs_ty = infer_expr_type(&bin.left, env, registry, func_registry)?;
            match &bin.op {
                syn::BinOp::AddAssign(_)
                | syn::BinOp::SubAssign(_)
                | syn::BinOp::MulAssign(_)
                | syn::BinOp::DivAssign(_)
                    if matches!(lhs_ty, GlslType::Array(_, _)) =>
                {
                    let lhs = generate_assignment_lhs(&bin.left, env, registry, func_registry)?;
                    emit_compound_array_assign(
                        &lhs,
                        &lhs_ty,
                        binary_operator_token(&bin.op)?,
                        &bin.left,
                        &bin.right,
                        env,
                        registry,
                        func_registry,
                        temp_counter,
                    )
                }
                syn::BinOp::AddAssign(_)
                | syn::BinOp::SubAssign(_)
                | syn::BinOp::MulAssign(_)
                | syn::BinOp::DivAssign(_) => {
                    let lhs = generate_assignment_lhs(&bin.left, env, registry, func_registry)?;
                    let (rhs_str, rhs_ty) =
                        generate_expr(&bin.right, env, registry, func_registry)?;
                    let rhs_str = coerce_expression_to_type(rhs_str, &rhs_ty, &lhs_ty)?;
                    Ok(format!(
                        "({lhs} {}= {rhs_str});\n",
                        binary_operator_token(&bin.op)?
                    ))
                }
                _ => {
                    let (expr_str, _) = generate_expr(expr, env, registry, func_registry)?;
                    Ok(format!("{expr_str};\n"))
                }
            }
        }
        _ => {
            let (expr_str, _) = generate_expr(expr, env, registry, func_registry)?;
            Ok(format!("{expr_str};\n"))
        }
    }
}

fn generate_tail_expr(
    expr: &syn::Expr,
    tail: Tail<'_>,
    env: &TypeEnv,
    registry: &StructRegistry,
    func_registry: &FuncRegistry,
    temp_counter: &mut usize,
) -> Result<String, TranspileError> {
    let expr_ty = infer_expr_type(expr, env, registry, func_registry)?;
    match tail {
        Tail::Return(_) if matches!(expr_ty, GlslType::Array(_, _)) => {
            let temp_name = next_temp_name(temp_counter);
            let mut out = format!("{};\n", expr_ty.render_decl(&temp_name));
            out.push_str(&emit_expr_into_target(
                &temp_name,
                &expr_ty,
                expr,
                env,
                registry,
                func_registry,
                temp_counter,
            )?);
            out.push_str(&format!("return {temp_name};\n"));
            Ok(out)
        }
        Tail::Assign(name) => {
            let target_ty = env
                .get(name)
                .ok_or_else(|| TranspileError::UnknownVariable(name.to_string()))?;
            if matches!(target_ty, GlslType::Array(_, _)) {
                emit_expr_into_target(
                    name,
                    target_ty,
                    expr,
                    env,
                    registry,
                    func_registry,
                    temp_counter,
                )
            } else {
                let (expr_str, expr_ty) = generate_expr(expr, env, registry, func_registry)?;
                let expr_str = coerce_expression_to_type(expr_str, &expr_ty, target_ty)?;
                Ok(format!("{name} = {expr_str};\n"))
            }
        }
        Tail::Return(target_ty) => {
            let (expr_str, expr_ty) = generate_expr(expr, env, registry, func_registry)?;
            let expr_str = coerce_expression_to_type(expr_str, &expr_ty, target_ty)?;
            Ok(format!("return {expr_str};\n"))
        }
        Tail::Discard => {
            let (expr_str, _) = generate_expr(expr, env, registry, func_registry)?;
            Ok(format!("{expr_str};\n"))
        }
    }
}

fn emit_expr_into_target(
    target: &str,
    target_ty: &GlslType,
    expr: &syn::Expr,
    env: &TypeEnv,
    registry: &StructRegistry,
    func_registry: &FuncRegistry,
    temp_counter: &mut usize,
) -> Result<String, TranspileError> {
    let mut indices = Vec::new();
    emit_expr_into_target_with_indices(
        target,
        target_ty,
        expr,
        env,
        registry,
        func_registry,
        temp_counter,
        &mut indices,
    )
}

fn emit_expr_into_target_with_indices(
    target: &str,
    target_ty: &GlslType,
    expr: &syn::Expr,
    env: &TypeEnv,
    registry: &StructRegistry,
    func_registry: &FuncRegistry,
    temp_counter: &mut usize,
    indices: &mut Vec<String>,
) -> Result<String, TranspileError> {
    match target_ty {
        GlslType::Array(inner, len) => {
            let idx = next_index_name(temp_counter);
            indices.push(idx.clone());
            let body = emit_expr_into_target_with_indices(
                &format!("{target}[{idx}]"),
                inner,
                expr,
                env,
                registry,
                func_registry,
                temp_counter,
                indices,
            )?;
            indices.pop();
            Ok(format!(
                "for (int {idx} = 0; {idx} < {len}; {idx}++) {{\n{}}}\n",
                indent_block(&body)
            ))
        }
        _ => {
            let (expr_str, expr_ty) =
                render_indexed_expr(expr, indices, env, registry, func_registry)?;
            let expr_str = coerce_expression_to_type(expr_str, &expr_ty, target_ty)?;
            Ok(format!("{target} = {expr_str};\n"))
        }
    }
}

fn emit_compound_array_assign(
    target: &str,
    target_ty: &GlslType,
    op: &'static str,
    lhs_expr: &syn::Expr,
    rhs_expr: &syn::Expr,
    env: &TypeEnv,
    registry: &StructRegistry,
    func_registry: &FuncRegistry,
    temp_counter: &mut usize,
) -> Result<String, TranspileError> {
    let mut indices = Vec::new();
    emit_compound_array_assign_with_indices(
        target,
        target_ty,
        op,
        lhs_expr,
        rhs_expr,
        env,
        registry,
        func_registry,
        temp_counter,
        &mut indices,
    )
}

fn emit_compound_array_assign_with_indices(
    target: &str,
    target_ty: &GlslType,
    op: &'static str,
    lhs_expr: &syn::Expr,
    rhs_expr: &syn::Expr,
    env: &TypeEnv,
    registry: &StructRegistry,
    func_registry: &FuncRegistry,
    temp_counter: &mut usize,
    indices: &mut Vec<String>,
) -> Result<String, TranspileError> {
    match target_ty {
        GlslType::Array(inner, len) => {
            let idx = next_index_name(temp_counter);
            indices.push(idx.clone());
            let body = emit_compound_array_assign_with_indices(
                &format!("{target}[{idx}]"),
                inner,
                op,
                lhs_expr,
                rhs_expr,
                env,
                registry,
                func_registry,
                temp_counter,
                indices,
            )?;
            indices.pop();
            Ok(format!(
                "for (int {idx} = 0; {idx} < {len}; {idx}++) {{\n{}}}\n",
                indent_block(&body)
            ))
        }
        _ => {
            let (rhs_str, _) =
                render_indexed_expr(rhs_expr, indices, env, registry, func_registry)?;
            Ok(format!("{target} = ({target} {op} {rhs_str});\n"))
        }
    }
}

fn render_indexed_expr(
    expr: &syn::Expr,
    indices: &[String],
    env: &TypeEnv,
    registry: &StructRegistry,
    func_registry: &FuncRegistry,
) -> Result<(String, GlslType), TranspileError> {
    match expr {
        syn::Expr::Paren(p) => {
            let (inner, ty) = render_indexed_expr(&p.expr, indices, env, registry, func_registry)?;
            Ok((format!("({inner})"), ty))
        }
        syn::Expr::Binary(bin) => match &bin.op {
            syn::BinOp::Add(_) | syn::BinOp::Sub(_) | syn::BinOp::Mul(_) | syn::BinOp::Div(_) => {
                let (left, left_ty) =
                    render_indexed_operand(&bin.left, indices, env, registry, func_registry)?;
                let (right, right_ty) =
                    render_indexed_operand(&bin.right, indices, env, registry, func_registry)?;
                Ok((
                    format!("({left} {} {right})", binary_operator_token(&bin.op)?),
                    ty::infer_arithmetic_type(&left_ty, &right_ty)?,
                ))
            }
            _ => {
                let (expr_str, expr_ty) = generate_expr(expr, env, registry, func_registry)?;
                Ok((
                    index_expr(expr_str, indices),
                    descend_type(expr_ty, indices.len())?,
                ))
            }
        },
        _ => {
            let (expr_str, expr_ty) = generate_expr(expr, env, registry, func_registry)?;
            Ok((
                index_expr(expr_str, indices),
                descend_type(expr_ty, indices.len())?,
            ))
        }
    }
}

fn render_indexed_operand(
    expr: &syn::Expr,
    indices: &[String],
    env: &TypeEnv,
    registry: &StructRegistry,
    func_registry: &FuncRegistry,
) -> Result<(String, GlslType), TranspileError> {
    let expr_ty = infer_expr_type(expr, env, registry, func_registry)?;
    if matches!(expr_ty, GlslType::Array(_, _)) {
        render_indexed_expr(expr, indices, env, registry, func_registry)
    } else {
        let (expr_str, ty) = generate_expr(expr, env, registry, func_registry)?;
        Ok((expr_str, ty))
    }
}

fn descend_type(mut ty: GlslType, depth: usize) -> Result<GlslType, TranspileError> {
    for _ in 0..depth {
        ty = ty
            .array_element()
            .cloned()
            .ok_or(TranspileError::UnsupportedSyntax(
                "array index depth exceeds operand rank",
            ))?;
    }
    Ok(ty)
}

fn index_expr(expr: String, indices: &[String]) -> String {
    if indices.is_empty() {
        expr
    } else {
        format!(
            "({expr}){}",
            indices
                .iter()
                .map(|idx| format!("[{idx}]"))
                .collect::<String>()
        )
    }
}

fn binary_operator_token(op: &syn::BinOp) -> Result<&'static str, TranspileError> {
    match op {
        syn::BinOp::Add(_) | syn::BinOp::AddAssign(_) => Ok("+"),
        syn::BinOp::Sub(_) | syn::BinOp::SubAssign(_) => Ok("-"),
        syn::BinOp::Mul(_) | syn::BinOp::MulAssign(_) => Ok("*"),
        syn::BinOp::Div(_) | syn::BinOp::DivAssign(_) => Ok("/"),
        _ => Err(TranspileError::UnsupportedSyntax("binary operator")),
    }
}

fn generate_assignment_lhs(
    expr: &syn::Expr,
    env: &TypeEnv,
    registry: &StructRegistry,
    func_registry: &FuncRegistry,
) -> Result<String, TranspileError> {
    match expr {
        syn::Expr::Unary(u) if matches!(u.op, syn::UnOp::Deref(_)) => {
            Ok(generate_expr(&u.expr, env, registry, func_registry)?.0)
        }
        _ => Ok(generate_expr(expr, env, registry, func_registry)?.0),
    }
}

fn next_temp_name(counter: &mut usize) -> String {
    let name = format!("__rs2glsl_tmp_array_{counter}");
    *counter += 1;
    name
}

fn next_index_name(counter: &mut usize) -> String {
    let name = format!("__rs2glsl_i{counter}");
    *counter += 1;
    name
}

pub(super) fn generate_match(
    match_expr: &syn::ExprMatch,
    tail: Tail<'_>,
    env: &mut TypeEnv,
    registry: &StructRegistry,
    func_registry: &FuncRegistry,
    aliases: &TypeAliasMap,
    temp_counter: &mut usize,
) -> Result<String, TranspileError> {
    let (cond_str, cond_ty) = generate_expr(&match_expr.expr, env, registry, func_registry)?;
    if !cond_ty.is_integer() {
        return Err(TranspileError::UnsupportedSyntax(
            "match expression requires an integer discriminant for GLSL switch",
        ));
    }

    let mut arms_out = String::new();
    for arm in &match_expr.arms {
        if arm.guard.is_some() {
            return Err(TranspileError::UnsupportedSyntax(
                "match arm guards are not supported",
            ));
        }

        let case_label = match_pattern_to_case_label(&arm.pat)?;

        let arm_body = match arm.body.as_ref() {
            syn::Expr::Block(block_expr) => {
                let mut arm_env = env.clone();
                generate_block(
                    &block_expr.block,
                    &mut arm_env,
                    registry,
                    func_registry,
                    aliases,
                    temp_counter,
                    tail,
                )?
            }
            expr => generate_tail_expr(expr, tail, env, registry, func_registry, temp_counter)?,
        };

        let needs_break = !matches!(tail, Tail::Return(_));
        if needs_break {
            let mut case_body = arm_body;
            case_body.push_str("break;\n");
            arms_out.push_str(&format!(
                "{case_label} {{\n{}}}\n",
                indent_block(&case_body)
            ));
        } else {
            arms_out.push_str(&format!("{case_label} {{\n{}}}\n", indent_block(&arm_body)));
        }
    }

    Ok(format!(
        "switch ({cond_str}) {{\n{}}}\n",
        indent_block(&arms_out)
    ))
}

fn match_pattern_to_case_label(pat: &syn::Pat) -> Result<String, TranspileError> {
    match pat {
        syn::Pat::Lit(expr_lit) => match &expr_lit.lit {
            syn::Lit::Int(i) => Ok(format!("case {}:", i.base10_digits())),
            _ => Err(TranspileError::UnsupportedSyntax(
                "match pattern must be an integer literal or _",
            )),
        },
        syn::Pat::Wild(_) => Ok("default:".to_string()),
        _ => Err(TranspileError::UnsupportedSyntax(
            "only integer literal patterns and _ are supported in match",
        )),
    }
}

fn infer_match_arm_type(
    match_expr: &syn::ExprMatch,
    env: &TypeEnv,
    registry: &StructRegistry,
    func_registry: &FuncRegistry,
) -> Result<GlslType, TranspileError> {
    let first_arm = match_expr
        .arms
        .first()
        .ok_or(TranspileError::UnsupportedSyntax(
            "match must have at least one arm",
        ))?;
    match first_arm.body.as_ref() {
        syn::Expr::Block(b) => infer_block_tail_type(&b.block, env, registry, func_registry),
        expr => infer_expr_type(expr, env, registry, func_registry),
    }
}
