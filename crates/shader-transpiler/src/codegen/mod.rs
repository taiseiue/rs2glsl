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

    // 定数
    let mut global_env = TypeEnv::new();
    let mut out = String::new();
    for node in &file.items {
        if let Item::Const(c) = node {
            let name = c.ident.to_string();
            if global_env.contains_key(&name) {
                return Err(TranspileError::DuplicateConst(name));
            }
            let (glsl, ty) = item::generate_const(c, &global_env, &registry)?;
            global_env.insert(name, ty);
            out.push_str(&glsl);
        }
    }

    // main_image
    for node in &file.items {
        if let Item::Fn(func) = node {
            if func.sig.ident == "main_image" {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(&item::generate_function(func, &global_env, &registry)?);
                return Ok(out);
            }
        }
    }

    Err(TranspileError::MainImageNotFound)
}
