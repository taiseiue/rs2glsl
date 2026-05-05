pub use glam::{Vec2, Vec3, Vec4};

use shader_macro::builtin;

// ── 三角関数 ────────────────────────────────────────────────────────────────

#[builtin("sin")]
pub fn sin(x: f32) -> f32 {
    x.sin()
}

#[builtin("cos")]
pub fn cos(x: f32) -> f32 {
    x.cos()
}

#[builtin("tan")]
pub fn tan(x: f32) -> f32 {
    x.tan()
}

#[builtin("asin")]
pub fn asin(x: f32) -> f32 {
    x.asin()
}

#[builtin("acos")]
pub fn acos(x: f32) -> f32 {
    x.acos()
}

#[builtin("atan")]
pub fn atan(x: f32) -> f32 {
    x.atan()
}

#[builtin("radians")]
pub fn radians(deg: f32) -> f32 {
    deg.to_radians()
}

#[builtin("degrees")]
pub fn degrees(rad: f32) -> f32 {
    rad.to_degrees()
}

// ── 指数・対数 ──────────────────────────────────────────────────────────────

#[builtin("sqrt")]
pub fn sqrt(x: f32) -> f32 {
    x.sqrt()
}

#[builtin("inversesqrt")]
pub fn inversesqrt(x: f32) -> f32 {
    1.0 / x.sqrt()
}

#[builtin("exp")]
pub fn exp(x: f32) -> f32 {
    x.exp()
}

#[builtin("exp2")]
pub fn exp2(x: f32) -> f32 {
    x.exp2()
}

#[builtin("log")]
pub fn log(x: f32) -> f32 {
    x.ln()
}

#[builtin("log2")]
pub fn log2(x: f32) -> f32 {
    x.log2()
}

#[builtin("pow")]
pub fn pow(x: f32, y: f32) -> f32 {
    x.powf(y)
}

// ── 共通スカラー関数 ────────────────────────────────────────────────────────

#[builtin("abs")]
pub fn abs(x: f32) -> f32 {
    x.abs()
}

#[builtin("sign")]
pub fn sign(x: f32) -> f32 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}

#[builtin("floor")]
pub fn floor(x: f32) -> f32 {
    x.floor()
}

#[builtin("ceil")]
pub fn ceil(x: f32) -> f32 {
    x.ceil()
}

#[builtin("round")]
pub fn round(x: f32) -> f32 {
    x.round()
}

#[builtin("fract")]
pub fn fract(x: f32) -> f32 {
    x.fract()
}

#[builtin("min")]
pub fn min(x: f32, y: f32) -> f32 {
    x.min(y)
}

#[builtin("max")]
pub fn max(x: f32, y: f32) -> f32 {
    x.max(y)
}

#[builtin("clamp")]
pub fn clamp(x: f32, lo: f32, hi: f32) -> f32 {
    x.clamp(lo, hi)
}

#[builtin("mix")]
pub fn mix(x: f32, y: f32, a: f32) -> f32 {
    x + (y - x) * a
}

/// GLSL の `mod(x, y)` に対応。Rust の `%` は符号が被除数に従うが、
/// GLSL は除数に従うため floor ベースで実装する。
#[builtin("mod")]
pub fn mod_(x: f32, y: f32) -> f32 {
    x - y * (x / y).floor()
}

#[builtin("smoothstep")]
pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// ── ベクトル関数 ─────────────────────────────────────────────────────────────
// GLSLはオーバーロード可だがRustは不可のため、型別に別名を使う。
// デフォルト名（サフィックスなし）は最も一般的な型。

#[builtin("length")]
pub fn length(v: Vec2) -> f32 {
    v.length()
}

#[builtin("length")]
pub fn length3(v: Vec3) -> f32 {
    v.length()
}

#[builtin("dot")]
pub fn dot(a: Vec3, b: Vec3) -> f32 {
    a.dot(b)
}

#[builtin("dot")]
pub fn dot2(a: Vec2, b: Vec2) -> f32 {
    a.dot(b)
}

#[builtin("cross")]
pub fn cross(a: Vec3, b: Vec3) -> Vec3 {
    a.cross(b)
}

#[builtin("normalize")]
pub fn normalize(v: Vec3) -> Vec3 {
    v.normalize()
}

#[builtin("normalize")]
pub fn normalize2(v: Vec2) -> Vec2 {
    v.normalize()
}

#[builtin("normalize")]
pub fn normalize4(v: Vec4) -> Vec4 {
    v.normalize()
}

#[builtin("distance")]
pub fn distance(a: Vec2, b: Vec2) -> f32 {
    (a - b).length()
}

#[builtin("distance")]
pub fn distance3(a: Vec3, b: Vec3) -> f32 {
    (a - b).length()
}

#[builtin("reflect")]
pub fn reflect(i: Vec3, n: Vec3) -> Vec3 {
    i - 2.0 * n.dot(i) * n
}
