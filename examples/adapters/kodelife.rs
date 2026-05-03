
// Kodelife用アダプタ
#[repr(vec2)]
struct Point {
    x: f32,
    y: f32,
}

#[repr(vec3)]
struct Color {
    r: f32,
    g: f32,
    b: f32,
}

#[uniform]
static time: f32;

#[uniform]
static resolution: Vec2;

#[uniform]
static spectrum: Vec3;

#[builtin("gl_FragCoord")]
static frag_coord_raw: Vec4;

#[builtin("mod")]
fn mod_(x: f32, y: f32) -> f32 {
    x
}

#[out]
#[glsl_name(fragColor)]
static mut frag_color: Vec4;

fn main() {
    let frag_coord = Point {
        x: frag_coord_raw.x,
        y: frag_coord_raw.y,
    };

    let res = Point {
        x: resolution.x,
        y: resolution.y,
    };

    let spec = Color {
        r: spectrum.x,
        g: spectrum.y,
        b: spectrum.z,
    };

    let col = pixel(frag_coord, res, spec, time);

    frag_color = vec4(col.r, col.g, col.b, 1.0);
}
