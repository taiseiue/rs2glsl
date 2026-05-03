mod codegen;
mod errors;
mod types;

pub use errors::TranspileError;

pub fn transpile_to_glsl(source: &str) -> Result<String, TranspileError> {
    // `static NAME: TYPE;`（値なし）はsynがパースできないので `= ()` を補完する
    let preprocessed = preprocess(source);
    let file = syn::parse_file(&preprocessed)
        .map_err(|e| TranspileError::ParseError(e.to_string()))?;
    codegen::generate(&file)
}

fn preprocess(source: &str) -> String {
    source.lines().map(|line| {
        let t = line.trim();
        if t.starts_with("static ") && t.ends_with(';') && !t.contains('=') {
            // `static NAME: TYPE;` → `static NAME: TYPE = ();`
            let pos = line.rfind(';').unwrap();
            format!("{} = ();", &line[..pos])
        } else {
            line.to_string()
        }
    }).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glsl(src: &str) -> String {
        transpile_to_glsl(src).expect("transpile failed")
    }

    // ── 正常系 ────────────────────────────────────────────────────────────

    #[test]
    fn simple_function() {
        let out = glsl("fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 { vec4(1.0, 0.0, 0.0, 1.0) }");
        assert_eq!(out, "vec4 main_image(vec2 frag_coord, vec2 resolution, float time) {\nreturn vec4(1.0, 0.0, 0.0, 1.0);\n}");
    }

    #[test]
    fn let_binding_infers_type() {
        let out = glsl("fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 { let uv = frag_coord / resolution; vec4(uv.x, uv.y, 0.0, 1.0) }");
        assert!(out.contains("vec2 uv = (frag_coord / resolution);"));
        assert!(out.contains("return vec4(uv.x, uv.y, 0.0, 1.0);"));
    }

    #[test]
    fn user_defined_helper_return_type_inferred() {
        let out = glsl("\
fn double(x: f32) -> f32 { x * 2.0 }
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 { vec4(double(time), 0.0, 0.0, 1.0) }");
        // double の戻り値型が float として推論されること
        assert!(out.contains("float double(float x)"));
        // helper と main_image の間に空行が入ること
        assert!(out.contains("}\n\nvec4 main_image"));
        // 呼び出し側で double(time) の型が float と推論され vec4 の引数になること
        assert!(out.contains("return vec4(double(time), 0.0, 0.0, 1.0);"));
    }

    #[test]
    fn constant_emitted_before_function() {
        let out = glsl("\
const PI: f32 = 3.14159;
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 { vec4(PI, 0.0, 0.0, 1.0) }");
        assert!(out.starts_with("const float PI = 3.14159;"));
        assert!(out.contains("const float PI = 3.14159;\n\nvec4 main_image"));
        assert!(out.contains("return vec4(PI, 0.0, 0.0, 1.0);"));
    }

    #[test]
    fn type_alias_transparent_in_output() {
        let out = glsl("\
type Color = Vec4;
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Color { vec4(1.0, 0.0, 0.0, 1.0) }");
        // 型エイリアス宣言は出力されず、戻り値は vec4 になる
        assert!(!out.contains("Color"));
        assert!(out.contains("vec4 main_image"));
    }

    #[test]
    fn void_function_uses_discard_tail() {
        let out = glsl("\
fn noop() { sin(1.0) }
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 { vec4(1.0, 0.0, 0.0, 1.0) }");
        // void 関数の末尾式は return なしで出力される
        assert!(out.contains("void noop()"));
        assert!(!out.contains("return sin(1.0)"));
        assert!(out.contains("sin(1.0);"));
    }

    #[test]
    fn if_expression_as_let_initializer() {
        let out = glsl("\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let x = if time > 1.0 { 1.0 } else { 0.0 };
    vec4(x, 0.0, 0.0, 1.0)
}");
        assert!(out.contains("float x;"));
        assert!(out.contains("if ((time > 1.0))"));
        assert!(out.contains("x = 1.0;"));
        assert!(out.contains("x = 0.0;"));
        assert!(out.contains("return vec4(x, 0.0, 0.0, 1.0);"));
    }

    #[test]
    fn struct_maps_to_glsl_constructor() {
        let out = glsl("\
#[repr(vec4)]
struct Color { r: f32, g: f32, b: f32, a: f32 }
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 }
}");
        assert!(out.contains("return vec4(1.0, 0.0, 0.0, 1.0);"));
    }

    #[test]
    fn no_trailing_blank_line_inside_function() {
        let out = glsl("fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 { vec4(1.0, 0.0, 0.0, 1.0) }");
        // '}' の直前に空行がないこと
        assert!(!out.contains("\n\n}"));
    }

    // ── ビルトイン変数 ────────────────────────────────────────────────────

    #[test]
    fn builtin_renames_to_glsl_name() {
        let out = glsl("\
#[builtin(iTime)]
static i_time: f32;
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    vec4(sin(i_time), 0.0, 0.0, 1.0)
}");
        // Rust名は消え、GLSL名で emit される
        assert!(!out.contains("i_time"));
        assert!(out.contains("sin(iTime)"));
    }

    #[test]
    fn builtin_vec3_swizzle() {
        let out = glsl("\
#[builtin(iResolution)]
static i_resolution: Vec3;
fn main_image(frag_color: &mut Vec4, frag_coord: Vec2) {
    let uv = frag_coord / i_resolution.xy;
    *frag_color = vec4(uv.x, uv.y, 0.0, 1.0);
}");
        assert!(out.contains("(frag_coord / iResolution.xy)"));
    }

    #[test]
    fn builtin_dotted_glsl_name() {
        let out = glsl("\
#[builtin(inData.v_texcoord)]
static v_texcoord: Vec2;
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    vec4(v_texcoord.x, v_texcoord.y, 0.0, 1.0)
}");
        // Rust 名 v_texcoord は GLSL 名 inData.v_texcoord に置換されること
        assert!(out.contains("inData.v_texcoord.x"));
        assert!(out.contains("inData.v_texcoord.y"));
        // 変数宣言として Rust 名が残らないこと
        assert!(!out.contains("vec2 v_texcoord"));
    }

    #[test]
    fn builtin_no_glsl_declaration_emitted() {
        let out = glsl("\
#[builtin(iTime)]
static i_time: f32;
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    vec4(i_time, 0.0, 0.0, 1.0)
}");
        // GLSL宣言は出力されない
        assert!(!out.contains("uniform"));
        assert!(!out.contains("static"));
    }

    // ── out パラメータ ────────────────────────────────────────────────────

    #[test]
    fn out_param_emits_qualifier() {
        let out = glsl("\
fn main_image(frag_color: &mut Vec4, frag_coord: Vec2, resolution: Vec2, time: f32) {
    *frag_color = vec4(1.0, 0.0, 0.0, 1.0);
}");
        assert!(out.contains("out vec4 frag_color"));
        // 通常パラメータには qualifier がつかない
        assert!(out.contains("vec2 frag_coord"));
        assert!(!out.contains("in vec2"));
    }

    #[test]
    fn deref_assign_strips_deref() {
        let out = glsl("\
fn main_image(frag_color: &mut Vec4, frag_coord: Vec2, resolution: Vec2, time: f32) {
    *frag_color = vec4(1.0, 0.0, 0.0, 1.0);
}");
        // *frag_color = ... → frag_color = ...（return なし、deref なし）
        assert!(out.contains("frag_color = vec4(1.0, 0.0, 0.0, 1.0);"));
        assert!(!out.contains("return"));
        assert!(!out.contains("*frag_color"));
    }

    #[test]
    fn deref_read_strips_deref() {
        let out = glsl("\
fn main_image(frag_color: &mut Vec4, src: Vec4) {
    *frag_color = *src;
}");
        // *src の読み取りも deref が消える
        assert!(out.contains("frag_color = src;"));
    }

    #[test]
    fn out_param_void_return() {
        let out = glsl("\
fn main_image(frag_color: &mut Vec4, frag_coord: Vec2, resolution: Vec2, time: f32) {
    *frag_color = vec4(1.0, 0.0, 0.0, 1.0);
}");
        assert!(out.starts_with("void main_image("));
    }

    // ── エラー系 ──────────────────────────────────────────────────────────

    #[test]
    fn error_duplicate_const() {
        let err = transpile_to_glsl("\
const X: f32 = 1.0;
const X: f32 = 2.0;
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 { vec4(1.0, 0.0, 0.0, 1.0) }")
            .unwrap_err();
        assert!(matches!(err, TranspileError::DuplicateConst(ref n) if n == "X"));
        assert_eq!(err.code(), "E0002");
    }

    #[test]
    fn error_unsupported_type() {
        let err = transpile_to_glsl("fn main_image(frag_coord: Vec2, resolution: Vec2, time: UnknownType) -> Vec4 { vec4(1.0, 0.0, 0.0, 1.0) }").unwrap_err();
        assert!(matches!(err, TranspileError::UnsupportedType(ref t) if t == "UnknownType"));
        assert_eq!(err.code(), "E0003");
    }

    #[test]
    fn error_unknown_variable() {
        let err = transpile_to_glsl("fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 { ghost }").unwrap_err();
        assert!(matches!(err, TranspileError::UnknownVariable(ref v) if v == "ghost"));
        assert_eq!(err.code(), "E0004");
    }

    #[test]
    fn error_parse_error() {
        let err = transpile_to_glsl("this is not rust @@@").unwrap_err();
        assert!(matches!(err, TranspileError::ParseError(_)));
        assert_eq!(err.code(), "E0007");
    }
}
