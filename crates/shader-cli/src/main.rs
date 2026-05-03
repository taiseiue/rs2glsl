use std::{env, fs, process};

fn main() {
    let paths = env::args().skip(1).collect::<Vec<_>>();
    if paths.is_empty() {
        eprintln!("usage: shader-cli <file.rs> [file.rs ...]");
        process::exit(1);
    }

    let source = read_sources(&paths).unwrap_or_else(|e| {
        eprintln!("{e}");
        process::exit(1);
    });

    match shader_transpiler::transpile_to_glsl(&source) {
        Ok(glsl) => print!("{glsl}"),
        Err(e) => {
            eprintln!("Error[{}]: {e}", e.code());
            process::exit(1);
        }
    }
}

fn read_sources(paths: &[String]) -> Result<String, String> {
    let mut source = String::new();

    for path in paths {
        let chunk =
            fs::read_to_string(path).map_err(|e| format!("File read error ({path}): {e}"))?;
        source.push_str(&chunk);
    }

    Ok(source)
}

#[cfg(test)]
mod tests {
    use super::read_sources;
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_file_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        std::env::temp_dir().join(format!("shader-cli-{name}-{nanos}.rs"))
    }

    #[test]
    fn reads_multiple_files_in_order() {
        let first = temp_file_path("first");
        let second = temp_file_path("second");

        fs::write(&first, "fn a() {}\n").expect("failed to write first file");
        fs::write(&second, "fn b() {}\n").expect("failed to write second file");

        let paths = vec![
            first.to_string_lossy().into_owned(),
            second.to_string_lossy().into_owned(),
        ];
        let source = read_sources(&paths).expect("failed to read sources");

        assert_eq!(source, "fn a() {}\nfn b() {}\n");

        fs::remove_file(first).expect("failed to remove first file");
        fs::remove_file(second).expect("failed to remove second file");
    }
}
