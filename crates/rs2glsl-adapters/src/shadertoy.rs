use rs2glsl_macros::{buildin};
use rs2glsl_prelude::*;

#[builtin("iResolution")]
static i_resolution: Vec3 = Vec3::new(0.0, 0.0, 0.0);

#[builtin("iTime")]
static i_time: f32 = 0.0;

fn mainImage(frag_color: &mut Vec4, frag_coord: Vec2) {
    let p = Point {
        x: frag_coord.x,
        y: frag_coord.y,
    };

    let res = Point {
        x: i_resolution.x,
        y: i_resolution.y,
    };

    let spec = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
    };

    let col = pixel(p, res, spec, i_time);

    *frag_color = Vec4::new(col.r, col.g, col.b, 1.0);
}
