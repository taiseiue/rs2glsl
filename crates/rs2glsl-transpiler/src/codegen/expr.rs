use super::structs::{StructRegistry, component_to_swizzle};
use super::ty;
use super::{FuncRegistry, TypeEnv};
use crate::errors::TranspileError;
use crate::types::GlslType;
use std::collections::HashMap;

pub(super) fn generate_expr(
    expr: &syn::Expr,
    env: &TypeEnv,
    registry: &StructRegistry,
    func_registry: &FuncRegistry,
) -> Result<(String, GlslType), TranspileError> {
    match expr {
        syn::Expr::Binary(bin) => {
            let (left, left_ty) = generate_expr(&bin.left, env, registry, func_registry)?;
            let (right, right_ty) = generate_expr(&bin.right, env, registry, func_registry)?;
            let (op, out_ty) = match &bin.op {
                syn::BinOp::Add(_) => ("+", ty::infer_binop_type(&left_ty, &right_ty)?),
                syn::BinOp::Sub(_) => ("-", ty::infer_binop_type(&left_ty, &right_ty)?),
                syn::BinOp::Mul(_) => ("*", ty::infer_binop_type(&left_ty, &right_ty)?),
                syn::BinOp::Div(_) => ("/", ty::infer_binop_type(&left_ty, &right_ty)?),
                syn::BinOp::AddAssign(_) => ("+=", left_ty.clone()),
                syn::BinOp::SubAssign(_) => ("-=", left_ty.clone()),
                syn::BinOp::MulAssign(_) => ("*=", left_ty.clone()),
                syn::BinOp::DivAssign(_) => ("/=", left_ty.clone()),
                syn::BinOp::Eq(_) => ("==", GlslType::Bool),
                syn::BinOp::Ne(_) => ("!=", GlslType::Bool),
                syn::BinOp::Lt(_) => ("<", GlslType::Bool),
                syn::BinOp::Gt(_) => (">", GlslType::Bool),
                syn::BinOp::Le(_) => ("<=", GlslType::Bool),
                syn::BinOp::Ge(_) => (">=", GlslType::Bool),
                syn::BinOp::And(_) => ("&&", GlslType::Bool),
                syn::BinOp::Or(_) => ("||", GlslType::Bool),
                _ => return Err(TranspileError::UnsupportedSyntax("binary operator")),
            };
            Ok((format!("({left} {op} {right})"), out_ty))
        }

        syn::Expr::Array(array) => {
            let elements = array
                .elems
                .iter()
                .map(|expr| generate_expr(expr, env, registry, func_registry))
                .collect::<Result<Vec<_>, _>>()?;
            build_array_literal(elements)
        }

        syn::Expr::Repeat(repeat) => {
            let len = ty::parse_array_len(&repeat.len)?;
            let (expr_str, expr_ty) = generate_expr(&repeat.expr, env, registry, func_registry)?;
            let elements = (0..len)
                .map(|_| (expr_str.clone(), expr_ty.clone()))
                .collect::<Vec<_>>();
            build_array_literal(elements)
        }

        syn::Expr::Call(call) => {
            let func_name = match &*call.func {
                syn::Expr::Path(p) => p
                    .path
                    .segments
                    .iter()
                    .map(|s| s.ident.to_string())
                    .collect::<Vec<_>>()
                    .join("::"),
                _ => return Err(TranspileError::UnsupportedSyntax("non-path function call")),
            };

            let (arg_strs, _): (Vec<_>, Vec<_>) = call
                .args
                .iter()
                .map(|a| generate_expr(a, env, registry, func_registry))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .unzip();

            let attrs = func_registry
                .get(&func_name)
                .ok_or_else(|| TranspileError::UndefinedFunction(func_name.clone()))?;
            let glsl_name = &attrs.glsl_name;
            // void 関数（return_type: None）を式として使った場合は Float で代替
            let out_ty = attrs.return_type.clone().unwrap_or(GlslType::Float);

            Ok((format!("{glsl_name}({})", arg_strs.join(", ")), out_ty))
        }

        syn::Expr::Struct(s) => {
            let struct_name = s.path.segments.last().unwrap().ident.to_string();
            let def = registry
                .get(&struct_name)
                .ok_or(TranspileError::UnsupportedSyntax(
                    "unknown struct in struct literal",
                ))?;

            // フィールド名から式のマップ
            let mut field_exprs: HashMap<String, String> = HashMap::new();
            for fv in &s.fields {
                let fname = match &fv.member {
                    syn::Member::Named(id) => id.to_string(),
                    _ => {
                        return Err(TranspileError::UnsupportedSyntax(
                            "unnamed field in struct literal",
                        ));
                    }
                };
                let (expr_str, _) = generate_expr(&fv.expr, env, registry, func_registry)?;
                field_exprs.insert(fname, expr_str);
            }

            let n = def.fields.len();
            let mut components: Vec<Option<String>> = vec![None; n];
            for (fname, &comp_idx) in &def.fields {
                if comp_idx >= n {
                    return Err(TranspileError::UnsupportedSyntax(
                        "component index out of range",
                    ));
                }
                let expr = field_exprs
                    .get(fname)
                    .ok_or(TranspileError::UnsupportedSyntax(
                        "missing field in struct literal",
                    ))?;
                components[comp_idx] = Some(expr.clone());
            }
            let args = components
                .into_iter()
                .map(|c| {
                    c.ok_or(TranspileError::UnsupportedSyntax(
                        "incomplete struct literal",
                    ))
                })
                .collect::<Result<Vec<_>, _>>()?;

            let constructor = def.glsl_type.to_glsl();
            let out_ty = GlslType::Struct(struct_name, Box::new(def.glsl_type.clone()));
            Ok((format!("{constructor}({})", args.join(", ")), out_ty))
        }

        syn::Expr::Field(field) => {
            let (base, base_ty) = generate_expr(&field.base, env, registry, func_registry)?;
            let member = match &field.member {
                syn::Member::Named(id) => id.to_string(),
                _ => return Err(TranspileError::UnsupportedSyntax("tuple field access")),
            };

            if let GlslType::Struct(struct_name, _) = &base_ty {
                let def = registry
                    .get(struct_name.as_str())
                    .ok_or(TranspileError::UnsupportedSyntax("unknown struct type"))?;
                let comp_idx = def
                    .fields
                    .get(&member)
                    .ok_or(TranspileError::UnsupportedSyntax("unknown struct field"))?;
                let swizzle = component_to_swizzle(*comp_idx)?;
                Ok((format!("{base}.{swizzle}"), GlslType::Float))
            } else {
                let out_ty = ty::infer_swizzle_type(&member)?;
                Ok((format!("{base}.{member}"), out_ty))
            }
        }

        syn::Expr::Index(index) => {
            let (base, base_ty) = generate_expr(&index.expr, env, registry, func_registry)?;
            let (idx, idx_ty) = generate_expr(&index.index, env, registry, func_registry)?;
            expect_int_index(&idx_ty)?;
            let element_ty = base_ty
                .array_element()
                .ok_or(TranspileError::UnsupportedSyntax(
                    "indexing a non-array expression",
                ))?
                .clone();
            Ok((format!("{base}[{idx}]"), element_ty))
        }

        syn::Expr::Path(p) => {
            let var_name = p.path.segments.last().unwrap().ident.to_string();
            let ty = env
                .get(&var_name)
                .ok_or_else(|| TranspileError::UnknownVariable(var_name.clone()))?
                .clone();
            // ビルトイン変数はRust名の代わりにGLSL名をemitする
            match ty {
                GlslType::Builtin(glsl_name, inner) => Ok((glsl_name, *inner)),
                _ => Ok((var_name, ty)),
            }
        }

        syn::Expr::Assign(a) => {
            let lhs_str = match &*a.left {
                syn::Expr::Unary(u) if matches!(u.op, syn::UnOp::Deref(_)) => {
                    generate_expr(&u.expr, env, registry, func_registry)?.0
                }
                e => generate_expr(e, env, registry, func_registry)?.0,
            };
            let (rhs_str, rhs_ty) = generate_expr(&a.right, env, registry, func_registry)?;
            Ok((format!("{lhs_str} = {rhs_str}"), rhs_ty))
        }

        syn::Expr::Cast(cast) => {
            let (expr_str, expr_ty) = generate_expr(&cast.expr, env, registry, func_registry)?;
            let target_ty = ty::parse_type(&cast.ty, registry, &Default::default())?;

            match (expr_ty.primitive(), target_ty.primitive()) {
                (GlslType::Int, GlslType::Float) => {
                    Ok((format!("float({expr_str})"), GlslType::Float))
                }
                (GlslType::Float, GlslType::Int) => Ok((format!("int({expr_str})"), GlslType::Int)),
                (src, dst) if src == dst => Ok((expr_str, target_ty)),
                _ => Err(TranspileError::UnsupportedSyntax(
                    "unsupported cast; only int <-> float casts are supported",
                )),
            }
        }

        syn::Expr::Unary(u) => {
            let (inner, inner_ty) = generate_expr(&u.expr, env, registry, func_registry)?;
            match &u.op {
                syn::UnOp::Neg(_) => Ok((format!("(-{inner})"), inner_ty)),
                syn::UnOp::Not(_) => Ok((format!("(!{inner})"), GlslType::Bool)),
                syn::UnOp::Deref(_) => Ok((inner, inner_ty)), // *x → x（deref を透過）
                _ => Err(TranspileError::UnsupportedSyntax("unary operator")),
            }
        }

        syn::Expr::Lit(lit) => match &lit.lit {
            syn::Lit::Float(f) => Ok((f.to_string(), GlslType::Float)),
            syn::Lit::Int(i) => Ok((i.to_string(), GlslType::Int)),
            syn::Lit::Bool(b) => Ok((b.value.to_string(), GlslType::Bool)),
            _ => Err(TranspileError::UnsupportedSyntax("literal kind")),
        },

        syn::Expr::Paren(p) => {
            let (inner, inner_ty) = generate_expr(&p.expr, env, registry, func_registry)?;
            Ok((format!("({inner})"), inner_ty))
        }

        _ => Err(TranspileError::UnsupportedSyntax("expression kind")),
    }
}

pub(super) fn extract_ident(pat: &syn::Pat) -> Result<String, TranspileError> {
    match pat {
        syn::Pat::Ident(i) => Ok(i.ident.to_string()),
        syn::Pat::Type(t) => extract_ident(&t.pat),
        _ => Err(TranspileError::UnsupportedSyntax("non-ident pattern")),
    }
}

fn expect_int_index(ty: &GlslType) -> Result<(), TranspileError> {
    if matches!(ty.primitive(), GlslType::Int) {
        Ok(())
    } else {
        Err(TranspileError::UnsupportedSyntax(
            "array index must be an integer",
        ))
    }
}

fn build_array_literal(
    elements: Vec<(String, GlslType)>,
) -> Result<(String, GlslType), TranspileError> {
    let mut iter = elements.into_iter();
    let (first_expr, element_ty) = iter.next().ok_or(TranspileError::UnsupportedSyntax(
        "GLSL does not support zero-length array literals",
    ))?;

    let mut exprs = vec![first_expr];
    for (expr_str, ty) in iter {
        if ty != element_ty {
            return Err(TranspileError::UnsupportedSyntax(
                "array elements must all have the same type",
            ));
        }
        exprs.push(expr_str);
    }

    let out_ty = GlslType::Array(Box::new(element_ty), exprs.len());
    let ctor_ty = out_ty.render_return_type();
    Ok((format!("{ctor_ty}({})", exprs.join(", ")), out_ty))
}
