fn pixel(
    frag_coord: Point,
    resolution: Point,
    spectrum: Color,
    time: f32,
) -> Color {
    let uv = Point {
        x: frag_coord.x / resolution.x,
        y: frag_coord.y / resolution.y,
    };

    Color {
        r: uv.x,
        g: uv.y,
        b: spectrum.r,
    }
}

// Shadertoy用アダプタ

#[repr(vec2)]
struct Point {
    x: f32,
    y: f32,
}

#[repr(vec3)]
struct Color {
    #[component(0)]
    r: f32,
    #[component(1)]
    g: f32,
    #[component(2)]
    b: f32,
}

#[builtin("iResolution")]
static i_resolution: Vec3 = vec3();

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

    *frag_color = vec4(col.r, col.g, col.b, 1.0);
}
