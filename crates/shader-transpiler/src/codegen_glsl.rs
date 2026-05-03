use std::collections::HashMap;
use syn::{File, Item};
use crate::types::GlslType;

type TypeEnv = HashMap<String, GlslType>;

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

    let body = generate_block(&func.block, &mut env);

    Ok(format!("{ret} {name}({args}) {{\n{body}\n}}"))
}

fn parse_type(ty: &syn::Type) -> GlslType {
    let ident = match ty {
        syn::Type::Path(p) => &p.path.segments.last().unwrap().ident,
        _ => panic!("unsupported type"),
    };
    match ident.to_string().as_str() {
        "f32" => GlslType::Float,
        "Vec2" => GlslType::Vec2,
        "Vec3" => GlslType::Vec3,
        "Vec4" => GlslType::Vec4,
        unknown => panic!("unknown type: {unknown}"),
    }
}

fn generate_block(block: &syn::Block, env: &mut TypeEnv) -> String {
    let mut out = String::new();

    for stmt in &block.stmts {
        match stmt {
            syn::Stmt::Local(local) => {
                let name = extract_ident(&local.pat);
                let (expr_str, ty) = generate_expr(local.init.as_ref().unwrap().expr.as_ref(), env);
                env.insert(name.clone(), ty.clone());
                out.push_str(&format!("{} {name} = {expr_str};\n", ty.to_glsl()));
            }

            syn::Stmt::Expr(expr, _) => {
                let (expr_str, _) = generate_expr(expr, env);
                out.push_str(&format!("return {expr_str};\n"));
            }

            _ => panic!("unsupported stmt"),
        }
    }

    out
}

fn generate_expr(expr: &syn::Expr, env: &TypeEnv) -> (String, GlslType) {
    match expr {
        syn::Expr::Binary(bin) => {
            let (left, left_ty) = generate_expr(&bin.left, env);
            let (right, right_ty) = generate_expr(&bin.right, env);
            let op = match &bin.op {
                syn::BinOp::Add(_) => "+",
                syn::BinOp::Sub(_) => "-",
                syn::BinOp::Mul(_) => "*",
                syn::BinOp::Div(_) => "/",
                _ => panic!("unsupported op"),
            };
            let ty = infer_binop_type(&left_ty, &right_ty);
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
        // 引数と同じ型を返す組み込み関数
        "sin" | "cos" | "tan" | "asin" | "acos" | "atan"
        | "sqrt" | "inversesqrt" | "abs" | "sign"
        | "floor" | "ceil" | "fract" | "round"
        | "exp" | "log" | "exp2" | "log2"
        | "radians" | "degrees" | "normalize"
        | "reflect" | "refract"
        | "min" | "max" | "mod" | "pow"
        | "mix" | "clamp" | "smoothstep" => first(),
        // 未知の関数はfloatと仮定する
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
