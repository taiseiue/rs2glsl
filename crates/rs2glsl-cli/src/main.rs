use std::{env, process};

fn main() {
    let paths = collect_paths(env::args());
    if paths.is_empty() {
        eprintln!("usage: cargo-rs2glsl <path> [path ...]");
        process::exit(1);
    }

    let source = rs2glsl_resolver::read_sources(&paths).unwrap_or_else(|e| {
        eprintln!("{e}");
        process::exit(1);
    });

    match rs2glsl_transpiler::transpile_to_glsl(&source) {
        Ok(glsl) => print!("{glsl}"),
        Err(e) => {
            if let Some(location) = e.location() {
                eprintln!(
                    "Error[{}] at line {}, column {}: {e}",
                    e.code(),
                    location.line,
                    location.column
                );
            } else {
                eprintln!("Error[{}]: {e}", e.code());
            }
            process::exit(1);
        }
    }
}

fn collect_paths(args: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut args = args.into_iter().skip(1).peekable();
    if args.peek().is_some_and(|arg| arg == "rs2glsl") {
        args.next();
    }
    args.collect()
}

#[cfg(test)]
mod tests {
    use super::collect_paths;

    #[test]
    fn collect_paths_for_direct_execution() {
        let args = vec![
            "cargo-rs2glsl".to_owned(),
            "examples/basic.rs".to_owned(),
            "examples/adapters/shadertoy.rs".to_owned(),
        ];

        assert_eq!(
            collect_paths(args),
            vec![
                "examples/basic.rs".to_owned(),
                "examples/adapters/shadertoy.rs".to_owned(),
            ]
        );
    }

    #[test]
    fn collect_paths_for_cargo_subcommand() {
        let args = vec![
            "cargo-rs2glsl".to_owned(),
            "rs2glsl".to_owned(),
            "examples/basic.rs".to_owned(),
            "examples/adapters/shadertoy.rs".to_owned(),
        ];

        assert_eq!(
            collect_paths(args),
            vec![
                "examples/basic.rs".to_owned(),
                "examples/adapters/shadertoy.rs".to_owned(),
            ]
        );
    }
}
