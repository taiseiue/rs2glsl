use std::collections::HashMap;
use crate::errors::TranspileError;
use crate::types::GlslType;

pub(super) struct StructDef {
    pub(super) glsl_type: GlslType,
    // フィールド名 → コンポーネントインデックス (0=x, 1=y, 2=z, 3=w)
    pub(super) fields: HashMap<String, usize>,
}

pub(super) type StructRegistry = HashMap<String, StructDef>;

pub(super) fn parse_struct(item: &syn::ItemStruct) -> Result<(String, StructDef), TranspileError> {
    let name = item.ident.to_string();
    let glsl_type = parse_repr_attr(item)?;
    let fields = parse_fields(item)?;
    Ok((name, StructDef { glsl_type, fields }))
}

fn parse_repr_attr(item: &syn::ItemStruct) -> Result<GlslType, TranspileError> {
    for attr in &item.attrs {
        if attr.path().is_ident("repr") {
            let ident: syn::Ident = attr.parse_args()
                .map_err(|_| TranspileError::UnsupportedSyntax("#[repr] requires a type name"))?;
            return match ident.to_string().as_str() {
                "vec2" => Ok(GlslType::Vec2),
                "vec3" => Ok(GlslType::Vec3),
                "vec4" => Ok(GlslType::Vec4),
                _ => Err(TranspileError::UnsupportedSyntax("#[repr] must be vec2, vec3, or vec4")),
            };
        }
    }
    Err(TranspileError::MissingReprAttr(item.ident.to_string()))
}

fn parse_fields(item: &syn::ItemStruct) -> Result<HashMap<String, usize>, TranspileError> {
    let syn::Fields::Named(named) = &item.fields else {
        return Err(TranspileError::UnsupportedSyntax("struct must have named fields"));
    };

    let mut fields = HashMap::new();
    for (decl_idx, field) in named.named.iter().enumerate() {
        if !is_f32(&field.ty) {
            return Err(TranspileError::UnsupportedSyntax("struct fields must be f32"));
        }
        let field_name = field.ident.as_ref().unwrap().to_string();
        let component = parse_component_attr(field).unwrap_or(decl_idx);
        fields.insert(field_name, component);
    }
    Ok(fields)
}

fn is_f32(ty: &syn::Type) -> bool {
    matches!(ty, syn::Type::Path(p) if p.path.is_ident("f32"))
}

fn parse_component_attr(field: &syn::Field) -> Option<usize> {
    for attr in &field.attrs {
        if attr.path().is_ident("component") {
            let lit: syn::LitInt = attr.parse_args().ok()?;
            return lit.base10_parse().ok();
        }
    }
    None
}

pub(super) fn component_to_swizzle(index: usize) -> Result<&'static str, TranspileError> {
    match index {
        0 => Ok("x"),
        1 => Ok("y"),
        2 => Ok("z"),
        3 => Ok("w"),
        _ => Err(TranspileError::UnsupportedSyntax("component index out of range (max 3)")),
    }
}
