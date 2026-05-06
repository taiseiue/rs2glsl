mod codegen;
mod errors;
mod types;

pub use errors::TranspileError;

pub fn transpile_to_glsl(source: &str) -> Result<String, TranspileError> {
    // `static NAME: TYPE;`（値なし）はsynがパースできないので `= ()` を補完する
    let preprocessed = preprocess(source);
    let file =
        syn::parse_file(&preprocessed).map_err(|e| TranspileError::ParseError(e.to_string()))?;
    codegen::generate(&file)
}

fn preprocess(source: &str) -> String {
    source
        .lines()
        .map(|line| {
            let t = line.trim();
            if t.starts_with("static ") && t.ends_with(';') && !t.contains('=') {
                // `static NAME: TYPE;` → `static NAME: TYPE = ();`
                let pos = line.rfind(';').unwrap();
                format!("{} = ();", &line[..pos])
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    // テスト用ミニマルプリルード。#[builtin] 関数はGLSL出力に影響しない。
    const TEST_PRELUDE: &str = concat!(
        "#[builtin(\"vec2\")] fn vec2(x: f32, y: f32) -> Vec2 {}\n",
        "#[builtin(\"vec3\")] fn vec3(x: f32, y: f32, z: f32) -> Vec3 {}\n",
        "#[builtin(\"vec4\")] fn vec4(x: f32, y: f32, z: f32, w: f32) -> Vec4 {}\n",
        "#[builtin(\"sin\")] fn sin(x: f32) -> f32 {}\n",
    );

    fn transpile(src: &str) -> Result<String, TranspileError> {
        transpile_to_glsl(&format!("{TEST_PRELUDE}{src}"))
    }

    fn glsl(src: &str) -> String {
        transpile(src).expect("transpile failed")
    }

    // ── 正常系 ────────────────────────────────────────────────────────────

    #[test]
    fn simple_function() {
        let out = glsl(
            "fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 { vec4(1.0, 0.0, 0.0, 1.0) }",
        );
        assert_eq!(
            out,
            "vec4 main_image(vec2 frag_coord, vec2 resolution, float time);\n\nvec4 main_image(vec2 frag_coord, vec2 resolution, float time) {\nreturn vec4(1.0, 0.0, 0.0, 1.0);\n}"
        );
    }

    #[test]
    fn let_binding_infers_type() {
        let out = glsl(
            "fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 { let uv = frag_coord / resolution; vec4(uv.x, uv.y, 0.0, 1.0) }",
        );
        assert!(out.contains("vec2 uv = (frag_coord / resolution);"));
        assert!(out.contains("return vec4(uv.x, uv.y, 0.0, 1.0);"));
    }

    #[test]
    fn user_defined_helper_return_type_inferred() {
        let out = glsl("\
fn double(x: f32) -> f32 { x * 2.0 }
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 { vec4(double(time), 0.0, 0.0, 1.0) }");
        assert!(out.contains("float double(float x);"));
        assert!(out.contains("vec4 main_image(vec2 frag_coord, vec2 resolution, float time);"));
        // double の戻り値型が float として推論されること
        assert!(out.contains("float double(float x) {"));
        // 呼び出し側で double(time) の型が float と推論され vec4 の引数になること
        assert!(out.contains("return vec4(double(time), 0.0, 0.0, 1.0);"));
    }

    #[test]
    fn function_call_can_target_later_definition() {
        let out = glsl(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    vec4(double(time), 0.0, 0.0, 1.0)
}
fn double(x: f32) -> f32 { x * 2.0 }",
        );
        assert!(out.starts_with(
            "vec4 main_image(vec2 frag_coord, vec2 resolution, float time);\nfloat double(float x);"
        ));
        assert!(out.contains("return vec4(double(time), 0.0, 0.0, 1.0);"));
        assert!(out.contains("float double(float x) {\nreturn (x * 2.0);\n}"));
    }

    #[test]
    fn constant_emitted_before_function() {
        let out = glsl(
            "\
const PI: f32 = 3.14159;
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 { vec4(PI, 0.0, 0.0, 1.0) }",
        );
        assert!(out.starts_with("const float PI = 3.14159;"));
        assert!(out.contains("const float PI = 3.14159;\n\nvec4 main_image(vec2 frag_coord, vec2 resolution, float time);"));
        assert!(out.contains("return vec4(PI, 0.0, 0.0, 1.0);"));
    }

    #[test]
    fn type_alias_transparent_in_output() {
        let out = glsl(
            "\
type Color = Vec4;
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Color { vec4(1.0, 0.0, 0.0, 1.0) }",
        );
        // 型エイリアス宣言は出力されず、戻り値は vec4 になる
        assert!(!out.contains("Color"));
        assert!(out.contains("vec4 main_image"));
    }

    #[test]
    fn void_function_uses_discard_tail() {
        let out = glsl(
            "\
fn noop() { sin(1.0) }
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 { vec4(1.0, 0.0, 0.0, 1.0) }",
        );
        // void 関数の末尾式は return なしで出力される
        assert!(out.contains("void noop()"));
        assert!(!out.contains("return sin(1.0)"));
        assert!(out.contains("sin(1.0);"));
    }

    #[test]
    fn if_expression_as_let_initializer() {
        let out = glsl(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let x = if time > 1.0 { 1.0 } else { 0.0 };
    vec4(x, 0.0, 0.0, 1.0)
}",
        );
        assert!(out.contains("float x;"));
        assert!(out.contains("if ((time > 1.0))"));
        assert!(out.contains("x = 1.0;"));
        assert!(out.contains("x = 0.0;"));
        assert!(out.contains("return vec4(x, 0.0, 0.0, 1.0);"));
    }

    #[test]
    fn if_expression_as_function_tail_returns_from_branches() {
        let out = glsl(
            "\
fn step(edge: f32, x: f32) -> f32 {
    if x < edge { 0.0 } else { 1.0 }
}",
        );
        assert!(out.contains("float step(float edge, float x)"));
        assert!(out.contains("if ((x < edge))"));
        assert!(out.contains("return 0.0;"));
        assert!(out.contains("return 1.0;"));
    }

    #[test]
    fn struct_maps_to_glsl_constructor() {
        let out = glsl(
            "\
#[structlayout(vec4)]
struct Color { r: f32, g: f32, b: f32, a: f32 }
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 }
}",
        );
        assert!(out.contains("return vec4(1.0, 0.0, 0.0, 1.0);"));
    }

    #[test]
    fn no_trailing_blank_line_inside_function() {
        let out = glsl(
            "fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 { vec4(1.0, 0.0, 0.0, 1.0) }",
        );
        // '}' の直前に空行がないこと
        assert!(!out.contains("\n\n}"));
    }

    // ── ビルトイン変数 ────────────────────────────────────────────────────

    #[test]
    fn builtin_renames_to_glsl_name() {
        let out = glsl(
            "\
#[builtin(\"iTime\")]
static i_time: f32;
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    vec4(sin(i_time), 0.0, 0.0, 1.0)
}",
        );
        // Rust名は消え、GLSL名で emit される
        assert!(!out.contains("i_time"));
        assert!(out.contains("sin(iTime)"));
    }

    #[test]
    fn builtin_vec3_swizzle() {
        let out = glsl(
            "\
#[builtin(\"iResolution\")]
static i_resolution: Vec3;
fn main_image(frag_color: &mut Vec4, frag_coord: Vec2) {
    let uv = frag_coord / i_resolution.xy;
    *frag_color = vec4(uv.x, uv.y, 0.0, 1.0);
}",
        );
        assert!(out.contains("(frag_coord / iResolution.xy)"));
    }

    #[test]
    fn builtin_dotted_glsl_name() {
        let out = glsl(
            "\
#[builtin(\"inData.v_texcoord\")]
static v_texcoord: Vec2;
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    vec4(v_texcoord.x, v_texcoord.y, 0.0, 1.0)
}",
        );
        // Rust 名 v_texcoord は GLSL 名 inData.v_texcoord に置換されること
        assert!(out.contains("inData.v_texcoord.x"));
        assert!(out.contains("inData.v_texcoord.y"));
        // 変数宣言として Rust 名が残らないこと
        assert!(!out.contains("vec2 v_texcoord"));
    }

    #[test]
    fn builtin_no_glsl_declaration_emitted() {
        let out = glsl(
            "\
#[builtin(\"iTime\")]
static i_time: f32;
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    vec4(i_time, 0.0, 0.0, 1.0)
}",
        );
        // GLSL宣言は出力されない
        assert!(!out.contains("uniform"));
        assert!(!out.contains("static"));
    }

    #[test]
    fn builtin_function_renames_call_and_skips_definition() {
        let out = glsl(
            "\
#[builtin(\"mod\")]
fn mod_(x: f32, y: f32) -> f32 { x }
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    vec4(mod_(time, 1.0), 0.0, 0.0, 1.0)
}",
        );
        assert!(!out.contains("float mod_(float x, float y)"));
        assert!(out.contains("return vec4(mod(time, 1.0), 0.0, 0.0, 1.0);"));
    }

    #[test]
    fn builtin_void_function_renames_statement_call() {
        let out = glsl(
            "\
#[builtin(\"barrier\")]
fn barrier_() {}
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    barrier_();
    vec4(1.0, 0.0, 0.0, 1.0)
}",
        );
        assert!(!out.contains("void barrier_()"));
        assert!(out.contains("barrier();"));
    }

    #[test]
    fn uniform_emits_glsl_declaration() {
        let out = glsl(
            "\
#[uniform]
static time: f32;
fn main_image(frag_coord: Vec2, resolution: Vec2) -> Vec4 {
    vec4(time, 0.0, 0.0, 1.0)
}",
        );
        assert!(out.starts_with("uniform float time;"));
        assert!(out.contains("return vec4(time, 0.0, 0.0, 1.0);"));
    }

    #[test]
    fn uniform_initializer_is_ignored() {
        let out = glsl(
            "\
#[uniform]
static time: f32 = 0.0;
fn main_image(frag_coord: Vec2, resolution: Vec2) -> Vec4 {
    vec4(time, 0.0, 0.0, 1.0)
}",
        );
        assert!(out.contains("uniform float time;"));
        assert!(!out.contains("= 0.0"));
    }

    #[test]
    fn out_static_emits_glsl_declaration() {
        let out = glsl(
            "\
#[out]
static mut fragColor: Vec4;
fn main_image(frag_coord: Vec2, resolution: Vec2) {
    fragColor = vec4(1.0, 0.0, 0.0, 1.0);
}",
        );
        assert!(out.starts_with("out vec4 fragColor;"));
        assert!(out.contains("fragColor = vec4(1.0, 0.0, 0.0, 1.0);"));
    }

    // ── out パラメータ ────────────────────────────────────────────────────

    #[test]
    fn out_param_emits_qualifier() {
        let out = glsl(
            "\
fn main_image(frag_color: &mut Vec4, frag_coord: Vec2, resolution: Vec2, time: f32) {
    *frag_color = vec4(1.0, 0.0, 0.0, 1.0);
}",
        );
        assert!(out.contains("out vec4 frag_color"));
        // 通常パラメータには qualifier がつかない
        assert!(out.contains("vec2 frag_coord"));
        assert!(!out.contains("in vec2"));
    }

    #[test]
    fn deref_assign_strips_deref() {
        let out = glsl(
            "\
fn main_image(frag_color: &mut Vec4, frag_coord: Vec2, resolution: Vec2, time: f32) {
    *frag_color = vec4(1.0, 0.0, 0.0, 1.0);
}",
        );
        // *frag_color = ... → frag_color = ...（return なし、deref なし）
        assert!(out.contains("frag_color = vec4(1.0, 0.0, 0.0, 1.0);"));
        assert!(!out.contains("return"));
        assert!(!out.contains("*frag_color"));
    }

    #[test]
    fn compound_assign_emits_glsl_operator() {
        let out = glsl(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> f32 {
    let mut value = time;
    value += 1.0;
    value *= 0.5;
    value
}",
        );
        assert!(out.contains("float value = time;"));
        assert!(out.contains("(value += 1.0);"));
        assert!(out.contains("(value *= 0.5);"));
        assert!(out.contains("return value;"));
    }

    #[test]
    fn compound_assign_on_field_keeps_member_access() {
        let out = glsl(
            "\
fn main_image(frag_color: &mut Vec4, src: Vec4) {
    frag_color.x += src.x;
}",
        );
        assert!(out.contains("(frag_color.x += src.x);"));
    }

    #[test]
    fn deref_read_strips_deref() {
        let out = glsl(
            "\
fn main_image(frag_color: &mut Vec4, src: Vec4) {
    *frag_color = *src;
}",
        );
        // *src の読み取りも deref が消える
        assert!(out.contains("frag_color = src;"));
    }

    #[test]
    fn out_param_void_return() {
        let out = glsl(
            "\
fn main_image(frag_color: &mut Vec4, frag_coord: Vec2, resolution: Vec2, time: f32) {
    *frag_color = vec4(1.0, 0.0, 0.0, 1.0);
}",
        );
        assert!(out.starts_with("void main_image("));
    }

    #[test]
    fn for_loop_over_half_open_range() {
        let out = glsl(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let acc = vec4(0.0, 0.0, 0.0, 1.0);
    for i in 0..3 {
        acc.x = acc.x + 1.0;
    }
    acc
}",
        );
        assert!(out.contains("for (int i = 0; i < 3; i++)"));
        assert!(out.contains("acc.x = (acc.x + 1.0);"));
        assert!(out.contains("return acc;"));
    }

    #[test]
    fn for_loop_over_closed_range() {
        let out = glsl(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let acc = vec4(0.0, 0.0, 0.0, 1.0);
    for i in 1..=2 {
        if i < 2 {
            acc.y = acc.y + 1.0;
        }
    }
    acc
}",
        );
        assert!(out.contains("for (int i = 1; i <= 2; i++)"));
        assert!(out.contains("if ((i < 2))"));
    }

    #[test]
    fn while_loop_with_comparison_condition() {
        let out = glsl(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let mut count = 0;
    while count < 3 {
        count += 1;
    }
    vec4(count as f32, 0.0, 0.0, 1.0)
}",
        );
        assert!(out.contains("while ((count < 3))"));
        assert!(out.contains("(count += 1)"));
    }

    #[test]
    fn while_loop_with_bool_variable() {
        let out = glsl(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let mut running = true;
    let mut x = 0.0;
    while running {
        x = x + 1.0;
        running = false;
    }
    vec4(x, 0.0, 0.0, 1.0)
}",
        );
        assert!(out.contains("while (running)"));
    }

    #[test]
    fn while_loop_with_break() {
        let out = glsl(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let mut i = 0;
    while i < 10 {
        if i == 5 {
            break;
        }
        i += 1;
    }
    vec4(i as f32, 0.0, 0.0, 1.0)
}",
        );
        assert!(out.contains("while ((i < 10))"));
        assert!(out.contains("break;"));
    }

    #[test]
    fn while_loop_with_continue() {
        let out = glsl(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let mut sum = 0;
    let mut i = 0;
    while i < 6 {
        i += 1;
        if i == 3 {
            continue;
        }
        sum += i;
    }
    vec4(sum as f32, 0.0, 0.0, 1.0)
}",
        );
        assert!(out.contains("while ((i < 6))"));
        assert!(out.contains("continue;"));
    }

    #[test]
    fn for_loop_with_break() {
        let out = glsl(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let mut acc = 0;
    for i in 0..10 {
        if i == 4 {
            break;
        }
        acc += 1;
    }
    vec4(acc as f32, 0.0, 0.0, 1.0)
}",
        );
        assert!(out.contains("for (int i = 0; i < 10; i++)"));
        assert!(out.contains("break;"));
    }

    #[test]
    fn loop_translates_to_while_true() {
        let out = glsl(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let mut i = 0;
    loop {
        if i >= 5 {
            break;
        }
        i += 1;
    }
    vec4(i as f32, 0.0, 0.0, 1.0)
}",
        );
        assert!(out.contains("while (true)"));
        assert!(out.contains("break;"));
    }

    #[test]
    fn loop_with_continue() {
        let out = glsl(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let mut i = 0;
    loop {
        i += 1;
        if i < 3 {
            continue;
        }
        break;
    }
    vec4(i as f32, 0.0, 0.0, 1.0)
}",
        );
        assert!(out.contains("while (true)"));
        assert!(out.contains("continue;"));
        assert!(out.contains("break;"));
    }

    #[test]
    fn error_labeled_break_is_unsupported() {
        let err = transpile(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let mut i = 0;
    'outer: while i < 5 {
        break 'outer;
    }
    vec4(0.0, 0.0, 0.0, 1.0)
}",
        )
        .unwrap_err();
        assert!(matches!(err, TranspileError::UnsupportedSyntax(_)));
    }

    #[test]
    fn array_let_initializer_and_index_access() {
        let out = glsl(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let weights: [f32; 3] = [1.0, 2.0, 3.0];
    vec4(weights[1], weights[2], 0.0, 1.0)
}",
        );
        assert!(out.contains("float weights[3];"));
        assert!(out.contains("weights[__rs2glsl_i"));
        assert!(out.contains("= (float[3](1.0, 2.0, 3.0))[__rs2glsl_i"));
        assert!(out.contains("return vec4(weights[1], weights[2], 0.0, 1.0);"));
    }

    #[test]
    fn array_parameter_is_emitted_with_size() {
        let out = glsl(
            "\
fn sum(values: [f32; 3]) -> f32 {
    values[0] + values[1] + values[2]
}
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    vec4(sum([1.0, 2.0, 3.0]), 0.0, 0.0, 1.0)
}",
        );
        assert!(out.contains("float sum(float values[3]);"));
        assert!(out.contains("return ((values[0] + values[1]) + values[2]);"));
        assert!(out.contains("return vec4(sum(float[3](1.0, 2.0, 3.0)), 0.0, 0.0, 1.0);"));
    }

    #[test]
    fn array_return_type_is_emitted_with_size() {
        let out = glsl(
            "\
fn weights() -> [f32; 3] {
    [1.0, 2.0, 3.0]
}
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let values = weights();
    vec4(values[0], values[1], values[2], 1.0)
}",
        );
        assert!(out.contains("float[3] weights();"));
        assert!(out.contains("float[3] weights() {"));
        assert!(out.contains("float __rs2glsl_tmp_array_0[3];"));
        assert!(out.contains("__rs2glsl_tmp_array_0[__rs2glsl_i"));
        assert!(out.contains("= (float[3](1.0, 2.0, 3.0))[__rs2glsl_i"));
        assert!(out.contains("return __rs2glsl_tmp_array_0;"));
        assert!(out.contains("float values[3];"));
        assert!(out.contains("values[__rs2glsl_i"));
        assert!(out.contains("= (weights())[__rs2glsl_i"));
    }

    #[test]
    fn multidimensional_array_literal_and_nested_index_access() {
        let out = glsl(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let grid: [[f32; 2]; 2] = [[1.0, 2.0], [3.0, 4.0]];
    vec4(grid[0][1], grid[1][0], 0.0, 1.0)
}",
        );
        assert!(out.contains("float grid[2][2];"));
        assert!(out.contains("grid[__rs2glsl_i"));
        assert!(
            out.contains("= (float[2][2](float[2](1.0, 2.0), float[2](3.0, 4.0)))[__rs2glsl_i")
        );
        assert!(out.contains("return vec4(grid[0][1], grid[1][0], 0.0, 1.0);"));
    }

    #[test]
    fn repeat_array_initializer_expands_constructor_arguments() {
        let out = glsl(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let values: [f32; 3] = [time; 3];
    vec4(values[0], values[1], values[2], 1.0)
}",
        );
        assert!(out.contains("float values[3];"));
        assert!(out.contains("values[__rs2glsl_i"));
        assert!(out.contains("= (float[3](time, time, time))[__rs2glsl_i"));
        assert!(out.contains("return vec4(values[0], values[1], values[2], 1.0);"));
    }

    #[test]
    fn repeat_initializer_supports_multidimensional_arrays() {
        let out = glsl(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let grid: [[f32; 2]; 2] = [[time; 2]; 2];
    vec4(grid[0][0], grid[0][1], grid[1][0], grid[1][1])
}",
        );
        assert!(out.contains("float grid[2][2];"));
        assert!(out.contains("grid[__rs2glsl_i"));
        assert!(
            out.contains("= (float[2][2](float[2](time, time), float[2](time, time)))[__rs2glsl_i")
        );
        assert!(out.contains("return vec4(grid[0][0], grid[0][1], grid[1][0], grid[1][1]);"));
    }

    #[test]
    fn array_addition_lowers_to_elementwise_loops() {
        let out = glsl(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let a: [f32; 3] = [1.0, 2.0, 3.0];
    let b: [f32; 3] = [4.0, 5.0, 6.0];
    let c = a + b;
    vec4(c[0], c[1], c[2], 1.0)
}",
        );
        assert!(out.contains("float c[3];"));
        assert!(out.contains("c[__rs2glsl_i"));
        assert!(out.contains("= ((a)[__rs2glsl_i"));
        assert!(out.contains("+ (b)[__rs2glsl_i"));
        assert!(out.contains("return vec4(c[0], c[1], c[2], 1.0);"));
    }

    #[test]
    fn array_scalar_multiplication_lowers_to_elementwise_loops() {
        let out = glsl(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let a: [f32; 3] = [1.0, 2.0, 3.0];
    let scaled = a * 2.0;
    vec4(scaled[0], scaled[1], scaled[2], 1.0)
}",
        );
        assert!(out.contains("scaled[__rs2glsl_i"));
        assert!(out.contains("= ((a)[__rs2glsl_i"));
        assert!(out.contains("* 2.0);"));
    }

    #[test]
    fn multidimensional_array_arithmetic_lowers_nested_loops() {
        let out = glsl(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let a: [[f32; 2]; 2] = [[1.0, 2.0], [3.0, 4.0]];
    let b: [[f32; 2]; 2] = [[5.0, 6.0], [7.0, 8.0]];
    let c = a + b;
    vec4(c[0][0], c[0][1], c[1][0], c[1][1])
}",
        );
        assert!(out.contains("for (int __rs2glsl_i"));
        assert!(out.contains("c[__rs2glsl_i"));
        assert!(out.contains("= ((a)[__rs2glsl_i"));
        assert!(out.contains("+ (b)[__rs2glsl_i"));
    }

    #[test]
    fn array_return_addition_lowers_via_temp() {
        let out = glsl(
            "\
fn sum(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    a + b
}
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let c = sum([1.0, 2.0, 3.0], [4.0, 5.0, 6.0]);
    vec4(c[0], c[1], c[2], 1.0)
}",
        );
        assert!(out.contains("float[3] sum(float a[3], float b[3]) {"));
        assert!(out.contains("float __rs2glsl_tmp_array_0[3];"));
        assert!(out.contains("__rs2glsl_tmp_array_0[__rs2glsl_i"));
        assert!(out.contains("= ((a)[__rs2glsl_i"));
        assert!(out.contains("+ (b)[__rs2glsl_i"));
        assert!(out.contains("return __rs2glsl_tmp_array_0;"));
    }

    #[test]
    fn array_compound_assign_lowers_to_elementwise_loops() {
        let out = glsl(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let values: [f32; 3] = [1.0, 2.0, 3.0];
    values += [4.0, 5.0, 6.0];
    vec4(values[0], values[1], values[2], 1.0)
}",
        );
        assert!(out.contains("values[__rs2glsl_i"));
        assert!(out.contains("= (values[__rs2glsl_i"));
        assert!(out.contains("+ (float[3](4.0, 5.0, 6.0))[__rs2glsl_i"));
    }

    #[test]
    fn array_equality_is_emitted_directly() {
        let out = glsl(
            "\
fn same(a: [f32; 3], b: [f32; 3]) -> bool {
    a == b
}",
        );
        assert!(out.contains("return (a == b);"));
    }

    #[test]
    fn array_index_assignment_keeps_subscript_on_lhs() {
        let out = glsl(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let values: [f32; 3] = [0.0, 1.0, 2.0];
    values[1] = 4.0;
    vec4(values[0], values[1], values[2], 1.0)
}",
        );
        assert!(out.contains("values[1] = 4.0;"));
        assert!(out.contains("return vec4(values[0], values[1], values[2], 1.0);"));
    }

    #[test]
    fn int_to_float_cast_uses_glsl_constructor() {
        let out = glsl(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let x = 2;
    vec4(x as f32, 0.0, 0.0, 1.0)
}",
        );
        assert!(out.contains("int x = 2;"));
        assert!(out.contains("return vec4(float(x), 0.0, 0.0, 1.0);"));
    }

    #[test]
    fn float_to_int_cast_uses_glsl_constructor() {
        let out = glsl(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let x = 2.5;
    let y = x as i32;
    vec4(y as f32, 0.0, 0.0, 1.0)
}",
        );
        assert!(out.contains("int y = int(x);"));
        assert!(out.contains("return vec4(float(y), 0.0, 0.0, 1.0);"));
    }

    #[test]
    fn same_type_cast_is_noop() {
        let out = glsl(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let x = 2;
    let y = x as i32;
    vec4(y as f32, 0.0, 0.0, 1.0)
}",
        );
        assert!(out.contains("int y = x;"));
    }

    #[test]
    fn uint_scalar_types_and_literals_are_emitted() {
        let out = glsl(
            "\
const COUNT: u32 = 1u32;
fn bump(x: u32) -> u32 {
    let y: u32 = 1;
    x + y + COUNT
}
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let count = bump(2u32);
    vec4(count as f32, 0.0, 0.0, 1.0)
}",
        );
        assert!(out.contains("const uint COUNT = 1u;"));
        assert!(out.contains("uint bump(uint x);"));
        assert!(out.contains("uint y = uint(1);"));
        assert!(out.contains("return ((x + y) + COUNT);"));
        assert!(out.contains("uint count = bump(2u);"));
        assert!(out.contains("return vec4(float(count), 0.0, 0.0, 1.0);"));
    }

    #[test]
    fn uint_for_loop_and_index_are_emitted() {
        let out = glsl(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let weights: [f32; 3] = [1.0, 2.0, 3.0];
    let idx: u32 = 1u32;
    let hit = 0.0;
    for i in 0u32..3u32 {
        if i == idx {
            hit = weights[i];
        }
    }
    vec4(hit, 0.0, 0.0, 1.0)
}",
        );
        assert!(out.contains("uint idx = 1u;"));
        assert!(out.contains("float hit = 0.0;"));
        assert!(out.contains("for (uint i = 0u; i < 3u; i++)"));
        assert!(out.contains("if ((i == idx))"));
        assert!(out.contains("hit = weights[i];"));
        assert!(out.contains("return vec4(hit, 0.0, 0.0, 1.0);"));
    }

    #[test]
    fn uint_assignments_and_compound_assignments_are_coerced() {
        let out = glsl(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let mut count: u32 = 1;
    count = 2;
    count += 3;
    vec4(count as f32, 0.0, 0.0, 1.0)
}",
        );
        assert!(out.contains("uint count = uint(1);"));
        assert!(out.contains("count = uint(2);"));
        assert!(out.contains("(count += uint(3));"));
        assert!(out.contains("return vec4(float(count), 0.0, 0.0, 1.0);"));
    }

    #[test]
    fn int_to_uint_cast_uses_glsl_constructor() {
        let out = glsl(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let x = 2;
    let y = x as u32;
    vec4(y as f32, 0.0, 0.0, 1.0)
}",
        );
        assert!(out.contains("uint y = uint(x);"));
        assert!(out.contains("return vec4(float(y), 0.0, 0.0, 1.0);"));
    }

    // ── エラー系 ──────────────────────────────────────────────────────────

    #[test]
    fn error_duplicate_const() {
        let err = transpile_to_glsl(
            "\
const X: f32 = 1.0;
const X: f32 = 2.0;
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 { vec4(1.0, 0.0, 0.0, 1.0) }",
        )
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
        let err = transpile_to_glsl(
            "fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 { ghost }",
        )
        .unwrap_err();
        assert!(matches!(err, TranspileError::UnknownVariable(ref v) if v == "ghost"));
        assert_eq!(err.code(), "E0004");
    }

    #[test]
    fn error_for_loop_iterable_must_be_range() {
        let err = transpile(
            "\
fn ints() -> i32 { 3 }
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let acc = vec4(0.0, 0.0, 0.0, 1.0);
    for i in ints() {
        acc.x = acc.x + 1.0;
    }
    acc
}",
        )
        .unwrap_err();
        assert!(matches!(
            err,
            TranspileError::UnsupportedSyntax("for loop iterable must be a range")
        ));
        assert_eq!(err.code(), "E0005");
    }

    #[test]
    fn error_for_loop_bounds_must_be_integers() {
        let err = transpile(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let acc = vec4(0.0, 0.0, 0.0, 1.0);
    for i in 0.0..3 {
        acc.x = acc.x + 1.0;
    }
    acc
}",
        )
        .unwrap_err();
        assert!(matches!(
            err,
            TranspileError::UnsupportedSyntax("for loop start bound must be an integer")
        ));
        assert_eq!(err.code(), "E0005");
    }

    #[test]
    fn error_cast_outside_int_float_pair() {
        let err = transpile(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let flag = true;
    vec4(flag as f32, 0.0, 0.0, 1.0)
}",
        )
        .unwrap_err();
        assert!(matches!(
            err,
            TranspileError::UnsupportedSyntax(
                "unsupported cast; only int/uint/float scalar casts are supported"
            )
        ));
        assert_eq!(err.code(), "E0005");
    }

    #[test]
    fn error_mixed_int_and_uint_arithmetic_is_rejected() {
        let err = transpile(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let x: u32 = 1u32;
    let y = x + 1;
    vec4(y as f32, 0.0, 0.0, 1.0)
}",
        )
        .unwrap_err();
        assert!(matches!(
            err,
            TranspileError::UnsupportedSyntax("arithmetic operands must have compatible numeric types")
        ));
        assert_eq!(err.code(), "E0005");
    }

    #[test]
    fn error_uint_negation_is_rejected() {
        let err = transpile(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let x: u32 = 1u32;
    vec4((-x) as f32, 0.0, 0.0, 1.0)
}",
        )
        .unwrap_err();
        assert!(matches!(
            err,
            TranspileError::UnsupportedSyntax("cannot negate a uint")
        ));
        assert_eq!(err.code(), "E0005");
    }

    #[test]
    fn error_array_index_must_be_integer() {
        let err = transpile(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let weights: [f32; 3] = [1.0, 2.0, 3.0];
    vec4(weights[1.0], 0.0, 0.0, 1.0)
}",
        )
        .unwrap_err();
        assert!(matches!(
            err,
            TranspileError::UnsupportedSyntax("array index must be an integer")
        ));
        assert_eq!(err.code(), "E0005");
    }

    #[test]
    fn error_empty_array_literal_is_rejected() {
        let err = transpile(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let values = [];
    vec4(1.0, 0.0, 0.0, 1.0)
}",
        )
        .unwrap_err();
        assert!(matches!(
            err,
            TranspileError::UnsupportedSyntax("GLSL does not support zero-length array literals")
        ));
        assert_eq!(err.code(), "E0005");
    }

    #[test]
    fn error_zero_length_array_type_is_rejected() {
        let err = transpile(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let values: [f32; 0] = [1.0; 0];
    vec4(1.0, 0.0, 0.0, 1.0)
}",
        )
        .unwrap_err();
        assert!(matches!(
            err,
            TranspileError::UnsupportedSyntax("GLSL does not support zero-length arrays")
        ));
        assert_eq!(err.code(), "E0005");
    }

    #[test]
    fn error_array_arithmetic_shape_mismatch_is_rejected() {
        let err = transpile(
            "\
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let a: [f32; 2] = [1.0, 2.0];
    let b: [f32; 3] = [3.0, 4.0, 5.0];
    let _c = a + b;
    vec4(1.0, 0.0, 0.0, 1.0)
}",
        )
        .unwrap_err();
        assert!(matches!(
            err,
            TranspileError::UnsupportedSyntax("array operands must have the same shape")
        ));
        assert_eq!(err.code(), "E0005");
    }

    #[test]
    fn error_builtin_and_uniform_cannot_be_combined() {
        let err = transpile_to_glsl(
            "\
#[builtin(\"iTime\")]
#[uniform]
static time: f32;
fn main_image(frag_coord: Vec2, resolution: Vec2) -> Vec4 {
    vec4(time, 0.0, 0.0, 1.0)
}",
        )
        .unwrap_err();
        assert!(matches!(
            err,
            TranspileError::UnsupportedSyntax(
                "#[builtin], #[uniform], and #[out] are mutually exclusive"
            )
        ));
        assert_eq!(err.code(), "E0005");
    }

    #[test]
    fn error_builtin_requires_string_literal() {
        let err = transpile_to_glsl(
            "\
#[builtin(iTime)]
static time: f32;
fn main_image(frag_coord: Vec2, resolution: Vec2) -> Vec4 {
    vec4(time, 0.0, 0.0, 1.0)
}",
        )
        .unwrap_err();
        assert!(matches!(
            err,
            TranspileError::UnsupportedSyntax(
                "#[builtin] requires a GLSL name string: #[builtin(\"iResolution\")]"
            )
        ));
        assert_eq!(err.code(), "E0005");
    }

    #[test]
    fn error_builtin_rejects_non_identifier_characters() {
        let err = transpile_to_glsl(
            "\
#[builtin(\"gl-FragCoord\")]
static frag_coord: Vec4;
fn main_image(frag_coord_in: Vec2, resolution: Vec2) -> Vec4 {
    frag_coord
}",
        )
        .unwrap_err();
        assert!(matches!(
            err,
            TranspileError::UnsupportedSyntax(
                "#[builtin] GLSL names must be dot-separated C identifiers"
            )
        ));
        assert_eq!(err.code(), "E0005");
    }

    #[test]
    fn error_builtin_function_requires_string_literal() {
        let err = transpile_to_glsl(
            "\
#[builtin(mod)]
fn mod_(x: f32, y: f32) -> f32 { x }
fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    vec4(mod_(time, 1.0), 0.0, 0.0, 1.0)
}",
        )
        .unwrap_err();
        assert!(matches!(
            err,
            TranspileError::UnsupportedSyntax(
                "#[builtin] requires a GLSL name string: #[builtin(\"iResolution\")]"
            )
        ));
        assert_eq!(err.code(), "E0005");
    }

    #[test]
    fn error_out_requires_static_mut() {
        let err = transpile_to_glsl(
            "\
#[out]
static fragColor: Vec4;
fn main_image(frag_coord: Vec2, resolution: Vec2) {
    fragColor = vec4(1.0, 0.0, 0.0, 1.0);
}",
        )
        .unwrap_err();
        assert!(matches!(
            err,
            TranspileError::UnsupportedSyntax("#[out] requires `static mut`")
        ));
        assert_eq!(err.code(), "E0005");
    }

    #[test]
    fn error_out_cannot_be_combined_with_uniform() {
        let err = transpile_to_glsl(
            "\
#[uniform]
#[out]
static mut fragColor: Vec4;
fn main_image(frag_coord: Vec2, resolution: Vec2) {
    fragColor = vec4(1.0, 0.0, 0.0, 1.0);
}",
        )
        .unwrap_err();
        assert!(matches!(
            err,
            TranspileError::UnsupportedSyntax(
                "#[builtin], #[uniform], and #[out] are mutually exclusive"
            )
        ));
        assert_eq!(err.code(), "E0005");
    }

    #[test]
    fn error_parse_error() {
        let err = transpile_to_glsl("this is not rust @@@").unwrap_err();
        assert!(matches!(err, TranspileError::ParseError(_)));
        assert_eq!(err.code(), "E0007");
    }
}
