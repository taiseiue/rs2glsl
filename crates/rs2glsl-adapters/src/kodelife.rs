use rs2glsl_macros::{builtin, glsl_name, out, uniform};
use rs2glsl_prelude::*;

#[uniform]
static time: f32 = 0.0;

#[uniform]
static resolution: Vec2 = ();

#[uniform]
static spectrum: Vec3 = ();

#[builtin("gl_FragCoord")]
static frag_coord_raw: Vec4 = ();

#[builtin("mod")]
fn mod_(x: f32, y: f32) -> f32 {
    x
}

#[out]
#[glsl_name(fragColor)]
static mut frag_color: Vec4 = ();

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

    frag_color = Vec4::new(col.r, col.g, col.b, 1.0);
}
