use crate::errors::TranspileError;
use crate::types::GlslType;
use std::collections::HashMap;
use syn::{File, Item};

mod expr;
mod item;
mod stmt;
mod structs;
mod ty;

type TypeEnv = HashMap<String, GlslType>;
type TypeAliasMap = HashMap<String, GlslType>;
type FuncRegistry = HashMap<String, FuncAttrs>;

#[derive(Clone)]
struct FuncAttrs {
    glsl_name: String,
    return_type: Option<GlslType>,
}

#[derive(Clone, Copy)]
enum Tail<'a> {
    Return,
    Assign(&'a str),
    Discard,
}

#[derive(Default)]
struct StaticAttrs {
    builtin: Option<String>,
    uniform: bool,
    out: bool,
}

#[derive(Default)]
struct FunctionAttrs {
    builtin: Option<String>,
}

fn is_valid_builtin_name(name: &str) -> bool {
    !name.is_empty()
        && name.split('.').all(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) if first == '_' || first.is_ascii_alphabetic() => {}
                _ => return false,
            }
            chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
        })
}

fn parse_builtin_attr(attrs: &[syn::Attribute]) -> Result<Option<String>, TranspileError> {
    let mut builtin = None;

    for attr in attrs {
        if attr.path().is_ident("builtin") {
            let name = attr
                .parse_args::<syn::LitStr>()
                .map_err(|_| {
                    TranspileError::UnsupportedSyntax(
                        "#[builtin] requires a GLSL name string: #[builtin(\"iResolution\")]",
                    )
                })?
                .value();
            if !is_valid_builtin_name(&name) {
                return Err(TranspileError::UnsupportedSyntax(
                    "#[builtin] GLSL names must be dot-separated C identifiers",
                ));
            }
            builtin = Some(name);
        }
    }

    Ok(builtin)
}

// #[builtin("GLSL名")] があれば Some(name)、なければ None、不正な形式なら Err
// GLSL名はドット区切りを許容し、各セグメントはC言語識別子として妥当である必要がある
fn parse_static_attrs(attrs: &[syn::Attribute]) -> Result<StaticAttrs, TranspileError> {
    let mut parsed = StaticAttrs::default();

    parsed.builtin = parse_builtin_attr(attrs)?;

    for attr in attrs {
        if attr.path().is_ident("uniform") {
            parsed.uniform = true;
        } else if attr.path().is_ident("out") {
            parsed.out = true;
        }
    }

    let attr_count = usize::from(parsed.builtin.is_some())
        + usize::from(parsed.uniform)
        + usize::from(parsed.out);
    if attr_count > 1 {
        return Err(TranspileError::UnsupportedSyntax(
            "#[builtin], #[uniform], and #[out] are mutually exclusive",
        ));
    }

    Ok(parsed)
}

fn parse_function_attrs(attrs: &[syn::Attribute]) -> Result<FunctionAttrs, TranspileError> {
    Ok(FunctionAttrs {
        builtin: parse_builtin_attr(attrs)?,
    })
}

pub fn generate(file: &File) -> Result<String, TranspileError> {
    // 構造体定義
    let mut registry = structs::StructRegistry::new();
    for node in &file.items {
        if let Item::Struct(s) = node {
            let (name, def) = structs::parse_struct(s)?;
            registry.insert(name, def);
        }
    }

    // 型エイリアス
    let mut aliases = TypeAliasMap::new();
    for node in &file.items {
        if let Item::Type(t) = node {
            let alias_name = t.ident.to_string();
            let glsl_type = ty::parse_type(&t.ty, &registry, &aliases)?;
            aliases.insert(alias_name, glsl_type);
        }
    }

    // 関数シグネチャ
    let mut func_registry = FuncRegistry::new();
    for (key, glsl_name, ty) in [
        ("Vec2::new", "vec2", GlslType::Vec2),
        ("Vec3::new", "vec3", GlslType::Vec3),
        ("Vec4::new", "vec4", GlslType::Vec4),
    ] {
        func_registry.insert(
            key.to_string(),
            FuncAttrs {
                glsl_name: glsl_name.to_string(),
                return_type: Some(ty),
            },
        );
    }
    for node in &file.items {
        if let Item::Fn(func) = node {
            let fn_name = func.sig.ident.to_string();
            let attrs = parse_function_attrs(&func.attrs)?;
            let return_type = match &func.sig.output {
                syn::ReturnType::Type(_, t) => Some(ty::parse_type(t, &registry, &aliases)?),
                syn::ReturnType::Default => None,
            };
            let glsl_name = attrs.builtin.clone().unwrap_or_else(|| fn_name.clone());
            func_registry.insert(
                fn_name,
                FuncAttrs {
                    glsl_name,
                    return_type,
                },
            );
        }
    }

    // グローバル変数
    let mut global_env = TypeEnv::new();
    let mut uniforms = Vec::new();
    let mut outputs = Vec::new();
    for node in &file.items {
        if let Item::Static(s) = node {
            let attrs = parse_static_attrs(&s.attrs)?;
            let rust_name = s.ident.to_string();
            let inner_ty = ty::parse_type(&s.ty, &registry, &aliases)?;

            if let Some(glsl_name) = attrs.builtin {
                global_env.insert(rust_name, GlslType::Builtin(glsl_name, Box::new(inner_ty)));
            } else if attrs.uniform {
                uniforms.push(format!("uniform {} {rust_name};\n", inner_ty.to_glsl()));
                global_env.insert(rust_name, inner_ty);
            } else if attrs.out {
                if matches!(s.mutability, syn::StaticMutability::None) {
                    return Err(TranspileError::UnsupportedSyntax(
                        "#[out] requires `static mut`",
                    ));
                }
                outputs.push(format!("out {} {rust_name};\n", inner_ty.to_glsl()));
                global_env.insert(rust_name, inner_ty);
            }
        }
    }

    let mut out = String::new();
    for uniform in uniforms {
        out.push_str(&uniform);
    }
    for output in outputs {
        out.push_str(&output);
    }

    // 定数
    for node in &file.items {
        if let Item::Const(c) = node {
            let name = c.ident.to_string();
            if global_env.contains_key(&name) {
                return Err(TranspileError::DuplicateConst(name));
            }
            let (glsl, ty) =
                item::generate_const(c, &global_env, &registry, &aliases, &func_registry)?;
            global_env.insert(name, ty);
            out.push_str(&glsl);
        }
    }

    let mut declarations = Vec::new();
    let mut functions = Vec::new();
    for node in &file.items {
        if let Item::Fn(func) = node {
            let attrs = parse_function_attrs(&func.attrs)?;
            if attrs.builtin.is_some() {
                continue;
            }
            let name = func.sig.ident.to_string();
            declarations.push(item::generate_function_declaration(
                func, &name, &registry, &aliases,
            )?);
            functions.push(item::generate_function(
                func,
                &name,
                &global_env,
                &registry,
                &aliases,
                &func_registry,
            )?);
        }
    }

    append_section(&mut out, &declarations, "\n");
    append_section(&mut out, &functions, "\n\n");

    Ok(out)
}

fn append_section(out: &mut String, items: &[String], separator: &str) {
    if items.is_empty() {
        return;
    }

    if !out.is_empty() {
        if out.ends_with('\n') {
            out.push('\n');
        } else {
            out.push_str("\n\n");
        }
    }

    out.push_str(&items.join(separator));
}
