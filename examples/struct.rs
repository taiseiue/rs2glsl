#[structlayout(vec4)]
struct Color {
    #[component(0)]
    r: f32,
    #[component(1)]
    g: f32,
    #[component(2)]
    b: f32,
    #[component(3)]
    a: f32,
}

#[structlayout(vec2)]
struct Uv {
    s: f32,
    t: f32,
}

fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let uv = Uv {
        s: frag_coord.x / resolution.x,
        t: frag_coord.y / resolution.y,
    };
    Color {
        r: uv.s,
        g: uv.t,
        b: sin(time),
        a: 1.0,
    }
}
