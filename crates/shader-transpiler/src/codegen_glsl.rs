use syn::{File, Item};

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
    let name = "mainImage"; // いったん狙いうちする

    let args = func.sig.inputs.iter().map(|arg| {
        match arg {
            syn::FnArg::Typed(pat) => {
                let name = extract_ident(&pat.pat);
                let ty = map_type(&pat.ty);
                format!("{ty} {name}")
            }
            _ => panic!("unsupported arg"),
        }
    }).collect::<Vec<_>>().join(", ");

    let ret = match &func.sig.output {
        syn::ReturnType::Type(_, ty) => map_type(ty),
        _ => "void".to_string(),
    };

    let body = generate_block(&func.block);

    Ok(format!("{ret} {name}({args}) {{\n{body}\n}}"))
}

fn map_type(ty: &syn::Type) -> String {
    let ident = match ty {
        syn::Type::Path(p) => &p.path.segments.last().unwrap().ident,
        _ => panic!("unsupported type"),
    };

    match ident.to_string().as_str() {
        "f32" => "float".into(),
        "Vec2" => "vec2".into(),
        "Vec3" => "vec3".into(),
        "Vec4" => "vec4".into(),
        _ => panic!("unknown type"),
    }
}

fn generate_block(block: &syn::Block) -> String {
    let mut out = String::new();

    for stmt in &block.stmts {
        match stmt {
            syn::Stmt::Local(local) => {
                let name = extract_ident(&local.pat);
                let expr = generate_expr(local.init.as_ref().unwrap().expr.as_ref());

                // 型は一旦vec2固定でもOK（後で直す）
                out.push_str(&format!("vec2 {name} = {expr};\n"));
            }

            syn::Stmt::Expr(expr, _) => {
                let expr_str = generate_expr(expr);
                out.push_str(&format!("return {expr_str};\n"));
            }

            _ => panic!("unsupported stmt"),
        }
    }

    out
}

fn generate_expr(expr: &syn::Expr) -> String {
    match expr {
        syn::Expr::Binary(bin) => {
            let left = generate_expr(&bin.left);
            let right = generate_expr(&bin.right);
            let op = match &bin.op {
                syn::BinOp::Add(_) => "+",
                syn::BinOp::Sub(_) => "-",
                syn::BinOp::Mul(_) => "*",
                syn::BinOp::Div(_) => "/",
                _ => panic!("unsupported op"),
            };
            format!("({left} {op} {right})")
        }

        syn::Expr::Call(call) => {
            let func = match &*call.func {
                syn::Expr::Path(p) => p.path.segments.last().unwrap().ident.to_string(),
                _ => panic!("unsupported call"),
            };

            let args = call.args.iter()
                .map(generate_expr)
                .collect::<Vec<_>>()
                .join(", ");

            format!("{func}({args})")
        }

        syn::Expr::Field(field) => {
            let base = generate_expr(&field.base);
            let member = match &field.member {
                syn::Member::Named(id) => id.to_string(),
                _ => panic!("unsupported field"),
            };

            format!("{base}.{member}")
        }

        syn::Expr::Path(p) => {
            p.path.segments.last().unwrap().ident.to_string()
        }

        syn::Expr::Lit(lit) => {
            match &lit.lit {
                syn::Lit::Float(f) => f.to_string(),
                syn::Lit::Int(i) => format!("{}.0", i),
                _ => panic!("unsupported literal"),
            }
        }

        _ => panic!("unsupported expr"),
    }
}

fn extract_ident(pat: &syn::Pat) -> String {
    match pat {
        syn::Pat::Ident(i) => i.ident.to_string(),
        _ => panic!("unsupported pat"),
    }
}
