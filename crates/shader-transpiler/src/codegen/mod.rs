use std::collections::HashMap;
use syn::{File, Item};
use crate::errors::TranspileError;
use crate::types::GlslType;

mod expr;
mod item;
mod stmt;
mod structs;
mod ty;

type TypeEnv = HashMap<String, GlslType>;
type TypeAliasMap = HashMap<String, GlslType>;
type FuncRegistry = HashMap<String, GlslType>;

#[derive(Clone, Copy)]
enum Tail<'a> {
    Return,
    Assign(&'a str),
    Discard,
}

// #[builtin(GLSL名)] があれば Some(name)、なければ None、不正な形式なら Err
// GLSL名はドット区切りも許容する (例: inData.v_texcoord)
fn find_builtin_attr(attrs: &[syn::Attribute]) -> Result<Option<String>, TranspileError> {
    for attr in attrs {
        if attr.path().is_ident("builtin") {
            let parts = attr.parse_args_with(
                syn::punctuated::Punctuated::<syn::Ident, syn::Token![.]>::parse_separated_nonempty
            ).map_err(|_| TranspileError::UnsupportedSyntax("#[builtin] requires a GLSL name: #[builtin(iResolution)]"))?;
            let name = parts.into_iter().map(|i| i.to_string()).collect::<Vec<_>>().join(".");
            return Ok(Some(name));
        }
    }
    Ok(None)
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

    // 関数シグネチャ (void 関数は登録しない)
    let mut func_registry = FuncRegistry::new();
    for node in &file.items {
        if let Item::Fn(func) = node {
            if let syn::ReturnType::Type(_, t) = &func.sig.output {
                let fn_name = func.sig.ident.to_string();
                let ret_ty = ty::parse_type(t, &registry, &aliases)?;
                func_registry.insert(fn_name, ret_ty);
            }
        }
    }

    // ビルトイン変数 (#[builtin(GLSL名)] static 名前: 型;)
    let mut global_env = TypeEnv::new();
    for node in &file.items {
        if let Item::Static(s) = node {
            if let Some(glsl_name) = find_builtin_attr(&s.attrs)? {
                let rust_name = s.ident.to_string();
                let inner_ty = ty::parse_type(&s.ty, &registry, &aliases)?;
                global_env.insert(rust_name, GlslType::Builtin(glsl_name, Box::new(inner_ty)));
            }
        }
    }

    // 定数
    let mut out = String::new();
    for node in &file.items {
        if let Item::Const(c) = node {
            let name = c.ident.to_string();
            if global_env.contains_key(&name) {
                return Err(TranspileError::DuplicateConst(name));
            }
            let (glsl, ty) = item::generate_const(c, &global_env, &registry, &aliases, &func_registry)?;
            global_env.insert(name, ty);
            out.push_str(&glsl);
        }
    }

    // 関数
    for node in &file.items {
        if let Item::Fn(func) = node {
            let name = func.sig.ident.to_string();
            if !out.is_empty() {
                // 直前が \n で終わっていれば1つ追加、そうでなければ2つ追加して空行を確保
                if out.ends_with('\n') {
                    out.push('\n');
                } else {
                    out.push_str("\n\n");
                }
            }
            out.push_str(&item::generate_function(func, &name, &global_env, &registry, &aliases, &func_registry)?);
        }
    }

    Ok(out)
}
