use crate::errors::TranspileError;
use crate::types::GlslType;
use super::structs::StructRegistry;
use super::{TypeAliasMap, FuncRegistry};

pub(super) fn parse_type(ty: &syn::Type, registry: &StructRegistry, aliases: &TypeAliasMap) -> Result<GlslType, TranspileError> {
    let ident = match ty {
        syn::Type::Path(p) => &p.path.segments.last().unwrap().ident,
        _ => return Err(TranspileError::UnsupportedSyntax("non-path type")),
    };
    match ident.to_string().as_str() {
        "bool" => Ok(GlslType::Bool),
        "f32"  => Ok(GlslType::Float),
        "Vec2" => Ok(GlslType::Vec2),
        "Vec3" => Ok(GlslType::Vec3),
        "Vec4" => Ok(GlslType::Vec4),
        name => {
            if let Some(glsl_ty) = aliases.get(name) {
                Ok(glsl_ty.clone())
            } else if let Some(def) = registry.get(name) {
                Ok(GlslType::Struct(name.to_string(), Box::new(def.glsl_type.clone())))
            } else {
                Err(TranspileError::UnsupportedType(name.to_string()))
            }
        }
    }
}

pub(super) fn infer_binop_type(left: &GlslType, right: &GlslType) -> GlslType {
    match (left.primitive(), right.primitive()) {
        (GlslType::Float, GlslType::Float) => GlslType::Float,
        (vec, GlslType::Float) => vec.clone(),
        (GlslType::Float, vec) => vec.clone(),
        (a, _) => a.clone(),
    }
}

pub(super) fn infer_call_type(func: &str, arg_types: &[GlslType], func_registry: &FuncRegistry) -> GlslType {
    let first = || arg_types.first().map(|t| t.primitive().clone()).unwrap_or(GlslType::Float);
    match func {
        "vec2" => GlslType::Vec2,
        "vec3" => GlslType::Vec3,
        "vec4" => GlslType::Vec4,
        "cross" => GlslType::Vec3,
        "length" | "dot" | "distance" => GlslType::Float,
        "sin" | "cos" | "tan" | "asin" | "acos" | "atan"
        | "sqrt" | "inversesqrt" | "abs" | "sign"
        | "floor" | "ceil" | "fract" | "round"
        | "exp" | "log" | "exp2" | "log2"
        | "radians" | "degrees" | "normalize"
        | "reflect" | "refract"
        | "min" | "max" | "mod" | "pow"
        | "mix" | "clamp" | "smoothstep" => first(),
        name => func_registry.get(name).cloned().unwrap_or(GlslType::Float),
    }
}

pub(super) fn infer_swizzle_type(member: &str) -> Result<GlslType, TranspileError> {
    match member.len() {
        1 => Ok(GlslType::Float),
        2 => Ok(GlslType::Vec2),
        3 => Ok(GlslType::Vec3),
        4 => Ok(GlslType::Vec4),
        _ => Err(TranspileError::UnsupportedSyntax("swizzle length exceeds 4")),
    }
}
