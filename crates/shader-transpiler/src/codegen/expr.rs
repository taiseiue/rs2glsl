use std::collections::HashMap;
use crate::errors::TranspileError;
use crate::types::GlslType;
use super::TypeEnv;
use super::structs::{component_to_swizzle, StructRegistry};
use super::ty;

pub(super) fn generate_expr(
    expr: &syn::Expr,
    env: &TypeEnv,
    registry: &StructRegistry,
) -> Result<(String, GlslType), TranspileError> {
    match expr {
        syn::Expr::Binary(bin) => {
            let (left, left_ty) = generate_expr(&bin.left, env, registry)?;
            let (right, right_ty) = generate_expr(&bin.right, env, registry)?;
            let (op, out_ty) = match &bin.op {
                syn::BinOp::Add(_) => ("+",  ty::infer_binop_type(&left_ty, &right_ty)),
                syn::BinOp::Sub(_) => ("-",  ty::infer_binop_type(&left_ty, &right_ty)),
                syn::BinOp::Mul(_) => ("*",  ty::infer_binop_type(&left_ty, &right_ty)),
                syn::BinOp::Div(_) => ("/",  ty::infer_binop_type(&left_ty, &right_ty)),
                syn::BinOp::Eq(_)  => ("==", GlslType::Bool),
                syn::BinOp::Ne(_)  => ("!=", GlslType::Bool),
                syn::BinOp::Lt(_)  => ("<",  GlslType::Bool),
                syn::BinOp::Gt(_)  => (">",  GlslType::Bool),
                syn::BinOp::Le(_)  => ("<=", GlslType::Bool),
                syn::BinOp::Ge(_)  => (">=", GlslType::Bool),
                _ => return Err(TranspileError::UnsupportedSyntax("binary operator")),
            };
            Ok((format!("({left} {op} {right})"), out_ty))
        }

        syn::Expr::Call(call) => {
            let func_name = match &*call.func {
                syn::Expr::Path(p) => p.path.segments.last().unwrap().ident.to_string(),
                _ => return Err(TranspileError::UnsupportedSyntax("non-path function call")),
            };

            let args_and_types = call.args.iter()
                .map(|a| generate_expr(a, env, registry))
                .collect::<Result<Vec<_>, _>>()?;
            let (arg_strs, arg_types): (Vec<_>, Vec<_>) = args_and_types.into_iter().unzip();

            let out_ty = ty::infer_call_type(&func_name, &arg_types);
            Ok((format!("{func_name}({})", arg_strs.join(", ")), out_ty))
        }

        syn::Expr::Struct(s) => {
            let struct_name = s.path.segments.last().unwrap().ident.to_string();
            let def = registry.get(&struct_name)
                .ok_or(TranspileError::UnsupportedSyntax("unknown struct in struct literal"))?;

            // フィールド名から式のマップ
            let mut field_exprs: HashMap<String, String> = HashMap::new();
            for fv in &s.fields {
                let fname = match &fv.member {
                    syn::Member::Named(id) => id.to_string(),
                    _ => return Err(TranspileError::UnsupportedSyntax("unnamed field in struct literal")),
                };
                let (expr_str, _) = generate_expr(&fv.expr, env, registry)?;
                field_exprs.insert(fname, expr_str);
            }

            let n = def.fields.len();
            let mut components: Vec<Option<String>> = vec![None; n];
            for (fname, &comp_idx) in &def.fields {
                if comp_idx >= n {
                    return Err(TranspileError::UnsupportedSyntax("component index out of range"));
                }
                let expr = field_exprs.get(fname)
                    .ok_or(TranspileError::UnsupportedSyntax("missing field in struct literal"))?;
                components[comp_idx] = Some(expr.clone());
            }
            let args = components.into_iter()
                .map(|c| c.ok_or(TranspileError::UnsupportedSyntax("incomplete struct literal")))
                .collect::<Result<Vec<_>, _>>()?;

            let constructor = def.glsl_type.to_glsl();
            let out_ty = GlslType::Struct(struct_name, Box::new(def.glsl_type.clone()));
            Ok((format!("{constructor}({})", args.join(", ")), out_ty))
        }

        syn::Expr::Field(field) => {
            let (base, base_ty) = generate_expr(&field.base, env, registry)?;
            let member = match &field.member {
                syn::Member::Named(id) => id.to_string(),
                _ => return Err(TranspileError::UnsupportedSyntax("tuple field access")),
            };

            if let GlslType::Struct(struct_name, _) = &base_ty {
                // 各フィールドをGLSLにする
                let def = registry.get(struct_name.as_str())
                    .ok_or(TranspileError::UnsupportedSyntax("unknown struct type"))?;
                let comp_idx = def.fields.get(&member)
                    .ok_or(TranspileError::UnsupportedSyntax("unknown struct field"))?;
                let swizzle = component_to_swizzle(*comp_idx)?;
                Ok((format!("{base}.{swizzle}"), GlslType::Float))
            } else {
                // vecのスウィズル
                let out_ty = ty::infer_swizzle_type(&member)?;
                Ok((format!("{base}.{member}"), out_ty))
            }
        }

        syn::Expr::Path(p) => {
            let var_name = p.path.segments.last().unwrap().ident.to_string();
            let out_ty = env.get(&var_name)
                .ok_or_else(|| TranspileError::UnknownVariable(var_name.clone()))?
                .clone();
            Ok((var_name, out_ty))
        }

        syn::Expr::Unary(u) => {
            let (inner, inner_ty) = generate_expr(&u.expr, env, registry)?;
            let (op, out_ty) = match &u.op {
                syn::UnOp::Neg(_) => ("-", inner_ty),
                syn::UnOp::Not(_) => ("!", GlslType::Bool),
                _ => return Err(TranspileError::UnsupportedSyntax("unary operator")),
            };
            Ok((format!("({op}{inner})"), out_ty))
        }

        syn::Expr::Lit(lit) => match &lit.lit {
            syn::Lit::Float(f) => Ok((f.to_string(), GlslType::Float)),
            syn::Lit::Int(i)   => Ok((format!("{}.0", i), GlslType::Float)),
            syn::Lit::Bool(b)  => Ok((b.value.to_string(), GlslType::Bool)),
            _ => Err(TranspileError::UnsupportedSyntax("literal kind")),
        },

        _ => Err(TranspileError::UnsupportedSyntax("expression kind")),
    }
}

pub(super) fn extract_ident(pat: &syn::Pat) -> Result<String, TranspileError> {
    match pat {
        syn::Pat::Ident(i) => Ok(i.ident.to_string()),
        _ => Err(TranspileError::UnsupportedSyntax("non-ident pattern")),
    }
}
