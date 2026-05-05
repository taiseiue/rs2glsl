mod resolver;

use std::{env, process};

fn main() {
    let paths = env::args().skip(1).collect::<Vec<_>>();
    if paths.is_empty() {
        eprintln!("usage: shader-cli <file.rs> [file.rs ...]");
        process::exit(1);
    }

    let source = resolver::read_sources(&paths).unwrap_or_else(|e| {
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

#[cfg(test)]
mod tests {
    use super::resolver::read_sources;
    use shader_transpiler::transpile_to_glsl;
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    const TEST_PRELUDE: &str =
        concat!("#[builtin(\"vec4\")] fn vec4(x: f32, y: f32, z: f32, w: f32) -> Vec4 {}\n",);

    fn temp_file_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        std::env::temp_dir().join(format!("shader-cli-{name}-{nanos}.rs"))
    }

    fn temp_dir_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        std::env::temp_dir().join(format!("shader-cli-{name}-{nanos}"))
    }

    fn write_file(path: &Path, source: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("failed to create parent directory");
        }
        fs::write(path, source).expect("failed to write file");
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

        assert!(source.contains("fn a"));
        assert!(source.contains("fn b"));

        fs::remove_file(first).expect("failed to remove first file");
        fs::remove_file(second).expect("failed to remove second file");
    }

    #[test]
    fn resolves_local_modules_and_flattens_imports() {
        let dir = temp_dir_path("module-flatten");
        let root = dir.join("shader.rs");
        let helper = dir.join("helper.rs");
        let math = dir.join("helper").join("math.rs");

        write_file(
            &root,
            "\
mod helper;
use crate::helper::*;

fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    vec4(helper(time), 0.0, 0.0, 1.0)
}
",
        );
        write_file(
            &helper,
            "\
mod math;
use crate::helper::math::*;

fn helper(x: f32) -> f32 {
    double(x)
}
",
        );
        write_file(
            &math,
            "\
fn double(x: f32) -> f32 {
    x * 2.0
}
",
        );

        let source = read_sources(&[root.to_string_lossy().into_owned()]).expect("resolve failed");
        assert!(source.contains("fn helper"));
        assert!(source.contains("fn double"));
        assert!(!source.contains("mod helper;"));
        assert!(!source.contains("use crate::helper::*;"));

        let glsl = transpile_to_glsl(&format!("{TEST_PRELUDE}{source}")).expect("transpile failed");
        assert!(glsl.contains("float double(float x);"));
        assert!(glsl.contains("float helper(float x);"));
        assert!(glsl.contains("return vec4(helper(time), 0.0, 0.0, 1.0);"));

        fs::remove_dir_all(dir).expect("failed to clean temp dir");
    }
}
