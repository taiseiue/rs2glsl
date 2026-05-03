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

    // 定数
    let mut global_env = TypeEnv::new();
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
    let mut found_main = false;
    for node in &file.items {
        if let Item::Fn(func) = node {
            let name = func.sig.ident.to_string();
            if name == "main_image" {
                found_main = true;
            }
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

    if found_main {
        Ok(out)
    } else {
        Err(TranspileError::MainImageNotFound)
    }
}
