use std::collections::HashMap;
use syn::{File, Item};
use crate::types::GlslType;

type TypeEnv = HashMap<String, GlslType>;

// 末尾式をどう出力するかのコンテキスト
enum Tail<'a> {
    Return,          // 関数末尾 → return expr;
    Assign(&'a str), // let x = if ... → x = expr;
    Discard,         // 単独 if 文のブランチ → expr;
}

pub fn generate(file: &File) -> Result<String, String> {
    for item in &file.items {
        if let Item::Fn(func) = item {
            if func.sig.ident == "main_image" {
                return generate_function(func);
            }
        }
    }
    Err("main_image not found".into())
}

fn generate_function(func: &syn::ItemFn) -> Result<String, String> {
    let name = "mainImage";
    let mut env = TypeEnv::new();

    let args = func.sig.inputs.iter().map(|arg| {
        match arg {
            syn::FnArg::Typed(pat) => {
                let param_name = extract_ident(&pat.pat);
                let ty = parse_type(&pat.ty);
                env.insert(param_name.clone(), ty.clone());
                format!("{} {param_name}", ty.to_glsl())
            }
            _ => panic!("unsupported arg"),
        }
    }).collect::<Vec<_>>().join(", ");

    let ret = match &func.sig.output {
        syn::ReturnType::Type(_, ty) => parse_type(ty).to_glsl().to_string(),
        _ => "void".to_string(),
    };

    let body = generate_block(&func.block, &mut env, Tail::Return);

    Ok(format!("{ret} {name}({args}) {{\n{body}\n}}"))
}

fn parse_type(ty: &syn::Type) -> GlslType {
    let ident = match ty {
        syn::Type::Path(p) => &p.path.segments.last().unwrap().ident,
        _ => panic!("unsupported type"),
    };
    match ident.to_string().as_str() {
        "bool" => GlslType::Bool,
        "f32" => GlslType::Float,
        "Vec2" => GlslType::Vec2,
        "Vec3" => GlslType::Vec3,
        "Vec4" => GlslType::Vec4,
        unknown => panic!("unknown type: {unknown}"),
    }
}

fn generate_block(block: &syn::Block, env: &mut TypeEnv, tail: Tail<'_>) -> String {
    let mut out = String::new();
    let stmts = &block.stmts;

    for (i, stmt) in stmts.iter().enumerate() {
        let is_last = i == stmts.len() - 1;

        match stmt {
            syn::Stmt::Local(local) => {
                let name = extract_ident(&local.pat);
                let init_expr = local.init.as_ref().unwrap().expr.as_ref();

                if let syn::Expr::If(if_expr) = init_expr {
                    // 前置ifなのでif+代入に展開
                    let ty = infer_block_tail_type(&if_expr.then_branch, env);
                    env.insert(name.clone(), ty.clone());
                    out.push_str(&format!("{} {name};\n", ty.to_glsl()));
                    out.push_str(&generate_if(if_expr, Tail::Assign(&name.clone()), env));
                } else {
                    let (expr_str, ty) = generate_expr(init_expr, env);
                    env.insert(name.clone(), ty.clone());
                    out.push_str(&format!("{} {name} = {expr_str};\n", ty.to_glsl()));
                }
            }

            syn::Stmt::Expr(expr, semi) => {
                if let syn::Expr::If(if_expr) = expr {
                    // 普通のif文
                    out.push_str(&generate_if(if_expr, Tail::Discard, env));
                } else if is_last && semi.is_none() {
                    // 末尾式
                    let (expr_str, _) = generate_expr(expr, env);
                    let line = match tail {
                        Tail::Return => format!("return {expr_str};\n"),
                        Tail::Assign(name) => format!("{name} = {expr_str};\n"),
                        Tail::Discard => format!("{expr_str};\n"),
                    };
                    out.push_str(&line);
                } else {
                    // 末尾ではなくセミコロンでもない
                    let (expr_str, _) = generate_expr(expr, env);
                    out.push_str(&format!("{expr_str};\n"));
                }
            }

            _ => panic!("unsupported stmt"),
        }
    }

    out
}

// if 式の then ブランチ末尾から型を推論する
fn infer_block_tail_type(block: &syn::Block, env: &TypeEnv) -> GlslType {
    let tail = block.stmts.iter().last()
        .unwrap_or_else(|| panic!("if branch must not be empty"));
    match tail {
        syn::Stmt::Expr(expr, None) => generate_expr(expr, env).1,
        _ => panic!("if branch must end with an expression"),
    }
}

fn generate_if(if_expr: &syn::ExprIf, tail: Tail<'_>, env: &mut TypeEnv) -> String {
    let (cond_str, _) = generate_expr(&if_expr.cond, env);
    let then_body = generate_block(&if_expr.then_branch, env, match tail {
        Tail::Return => Tail::Return,
        Tail::Assign(n) => Tail::Assign(n),
        Tail::Discard => Tail::Discard,
    });

    let else_str = match &if_expr.else_branch {
        None => String::new(),
        Some((_, else_expr)) => match else_expr.as_ref() {
            syn::Expr::Block(b) => {
                let else_tail = match tail {
                    Tail::Return => Tail::Return,
                    Tail::Assign(n) => Tail::Assign(n),
                    Tail::Discard => Tail::Discard,
                };
                let body = generate_block(&b.block, env, else_tail);
                format!(" else {{\n{body}}}")
            }
            syn::Expr::If(nested) => {
                let nested_tail = match tail {
                    Tail::Return => Tail::Return,
                    Tail::Assign(n) => Tail::Assign(n),
                    Tail::Discard => Tail::Discard,
                };
                format!(" else {}", generate_if(nested, nested_tail, env))
            }
            _ => panic!("unsupported else branch"),
        },
    };

    format!("if ({cond_str}) {{\n{then_body}}}{else_str}\n")
}

fn generate_expr(expr: &syn::Expr, env: &TypeEnv) -> (String, GlslType) {
    match expr {
        syn::Expr::Binary(bin) => {
            let (left, left_ty) = generate_expr(&bin.left, env);
            let (right, right_ty) = generate_expr(&bin.right, env);
            let (op, ty) = match &bin.op {
                syn::BinOp::Add(_) => ("+", infer_binop_type(&left_ty, &right_ty)),
                syn::BinOp::Sub(_) => ("-", infer_binop_type(&left_ty, &right_ty)),
                syn::BinOp::Mul(_) => ("*", infer_binop_type(&left_ty, &right_ty)),
                syn::BinOp::Div(_) => ("/", infer_binop_type(&left_ty, &right_ty)),
                syn::BinOp::Eq(_)  => ("==", GlslType::Bool),
                syn::BinOp::Ne(_)  => ("!=", GlslType::Bool),
                syn::BinOp::Lt(_)  => ("<",  GlslType::Bool),
                syn::BinOp::Gt(_)  => (">",  GlslType::Bool),
                syn::BinOp::Le(_)  => ("<=", GlslType::Bool),
                syn::BinOp::Ge(_)  => (">=", GlslType::Bool),
                _ => panic!("unsupported op"),
            };
            (format!("({left} {op} {right})"), ty)
        }

        syn::Expr::Call(call) => {
            let func_name = match &*call.func {
                syn::Expr::Path(p) => p.path.segments.last().unwrap().ident.to_string(),
                _ => panic!("unsupported call"),
            };

            let (arg_strs, arg_types): (Vec<_>, Vec<_>) = call.args.iter()
                .map(|a| generate_expr(a, env))
                .unzip();

            let ty = infer_call_type(&func_name, &arg_types);
            (format!("{func_name}({})", arg_strs.join(", ")), ty)
        }

        syn::Expr::Field(field) => {
            let (base, _) = generate_expr(&field.base, env);
            let member = match &field.member {
                syn::Member::Named(id) => id.to_string(),
                _ => panic!("unsupported field"),
            };
            let ty = infer_swizzle_type(&member);
            (format!("{base}.{member}"), ty)
        }

        syn::Expr::Path(p) => {
            let var_name = p.path.segments.last().unwrap().ident.to_string();
            let ty = env.get(&var_name)
                .unwrap_or_else(|| panic!("unknown variable: {var_name}"))
                .clone();
            (var_name, ty)
        }

        syn::Expr::Lit(lit) => {
            match &lit.lit {
                syn::Lit::Float(f) => (f.to_string(), GlslType::Float),
                syn::Lit::Int(i) => (format!("{}.0", i), GlslType::Float),
                syn::Lit::Bool(b) => (b.value.to_string(), GlslType::Bool),
                _ => panic!("unsupported literal"),
            }
        }

        _ => panic!("unsupported expr"),
    }
}

fn infer_binop_type(left: &GlslType, right: &GlslType) -> GlslType {
    match (left, right) {
        (GlslType::Float, GlslType::Float) => GlslType::Float,
        (vec, GlslType::Float) => vec.clone(),
        (GlslType::Float, vec) => vec.clone(),
        (a, _) => a.clone(),
    }
}

fn infer_call_type(func: &str, arg_types: &[GlslType]) -> GlslType {
    let first = || arg_types.first().cloned().unwrap_or(GlslType::Float);
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
        _ => GlslType::Float,
    }
}

fn infer_swizzle_type(member: &str) -> GlslType {
    match member.len() {
        1 => GlslType::Float,
        2 => GlslType::Vec2,
        3 => GlslType::Vec3,
        4 => GlslType::Vec4,
        _ => panic!("unsupported swizzle: {member}"),
    }
}

fn extract_ident(pat: &syn::Pat) -> String {
    match pat {
        syn::Pat::Ident(i) => i.ident.to_string(),
        _ => panic!("unsupported pat"),
    }
}
