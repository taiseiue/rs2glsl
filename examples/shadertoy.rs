// この関数がユーザーコード
fn pixel(uv: Point, time: f32) -> Color {
    let c = 0.5 + 0.5 * cos(time + uv.x + 0.0);
    Color {
        r: c,
        g: uv.y,
        b: 1.0,
    }
}

// これ以下はadapter
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
static i_time: f32 = 0;
fn mainImage(frag_color: &mut Vec4, frag_coord: Vec2) {
    let uv = frag_coord / i_resolution.xy;

    let p = Point { x: uv.x, y: uv.y };

    let col = pixel(p, i_time);

    *frag_color = vec4(col.r, col.g, col.b, 1.0);
}
