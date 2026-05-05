use super::TypeAliasMap;
use super::structs::StructRegistry;
use crate::errors::TranspileError;
use crate::types::GlslType;

// &mut T: (T, true=out)、それ以外:(T, false)
pub(super) fn parse_param_type(
    ty: &syn::Type,
    registry: &StructRegistry,
    aliases: &TypeAliasMap,
) -> Result<(GlslType, bool), TranspileError> {
    match ty {
        syn::Type::Reference(r) if r.mutability.is_some() => {
            Ok((parse_type(&r.elem, registry, aliases)?, true))
        }
        _ => Ok((parse_type(ty, registry, aliases)?, false)),
    }
}

pub(super) fn parse_type(
    ty: &syn::Type,
    registry: &StructRegistry,
    aliases: &TypeAliasMap,
) -> Result<GlslType, TranspileError> {
    if let syn::Type::Array(array) = ty {
        let inner = parse_type(&array.elem, registry, aliases)?;
        return Ok(GlslType::Array(
            Box::new(inner),
            parse_array_len(&array.len)?,
        ));
    }

    let ident = match ty {
        syn::Type::Path(p) => &p.path.segments.last().unwrap().ident,
        _ => return Err(TranspileError::UnsupportedSyntax("non-path type")),
    };
    match ident.to_string().as_str() {
        "bool" => Ok(GlslType::Bool),
        "i32" => Ok(GlslType::Int),
        "f32" => Ok(GlslType::Float),
        "Vec2" => Ok(GlslType::Vec2),
        "Vec3" => Ok(GlslType::Vec3),
        "Vec4" => Ok(GlslType::Vec4),
        name => {
            if let Some(glsl_ty) = aliases.get(name) {
                Ok(glsl_ty.clone())
            } else if let Some(def) = registry.get(name) {
                Ok(GlslType::Struct(
                    name.to_string(),
                    Box::new(def.glsl_type.clone()),
                ))
            } else {
                Err(TranspileError::UnsupportedType(name.to_string()))
            }
        }
    }
}

pub(super) fn parse_array_len(len: &syn::Expr) -> Result<usize, TranspileError> {
    match len {
        syn::Expr::Lit(expr_lit) => match &expr_lit.lit {
            syn::Lit::Int(int) => {
                let len = int.base10_parse().map_err(|_| {
                    TranspileError::UnsupportedSyntax("array length must fit in usize")
                })?;
                if len == 0 {
                    Err(TranspileError::UnsupportedSyntax(
                        "GLSL does not support zero-length arrays",
                    ))
                } else {
                    Ok(len)
                }
            }
            _ => Err(TranspileError::UnsupportedSyntax(
                "array length must be an integer literal",
            )),
        },
        _ => Err(TranspileError::UnsupportedSyntax(
            "array length must be an integer literal",
        )),
    }
}

pub(super) fn infer_binop_type(
    left: &GlslType,
    right: &GlslType,
) -> Result<GlslType, TranspileError> {
    if matches!(left, GlslType::Array(_, _)) || matches!(right, GlslType::Array(_, _)) {
        return Err(TranspileError::UnsupportedSyntax(
            "array binary operators are not supported",
        ));
    }

    match (left.primitive(), right.primitive()) {
        (GlslType::Int, GlslType::Int) => Ok(GlslType::Int),
        (GlslType::Float, GlslType::Float) => Ok(GlslType::Float),
        (GlslType::Float, GlslType::Int) => Ok(GlslType::Float),
        (GlslType::Int, GlslType::Float) => Ok(GlslType::Float),
        (vec, GlslType::Float) => Ok(vec.clone()),
        (GlslType::Float, vec) => Ok(vec.clone()),
        (a, _) => Ok(a.clone()),
    }
}

pub(super) fn infer_swizzle_type(member: &str) -> Result<GlslType, TranspileError> {
    match member.len() {
        1 => Ok(GlslType::Float),
        2 => Ok(GlslType::Vec2),
        3 => Ok(GlslType::Vec3),
        4 => Ok(GlslType::Vec4),
        _ => Err(TranspileError::UnsupportedSyntax(
            "swizzle length exceeds 4",
        )),
    }
}
