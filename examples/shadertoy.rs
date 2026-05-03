#[repr(vec3)]
struct Color {
    #[component(0)]
    r: f32,
    #[component(1)]
    g: f32,
    #[component(2)]
    b: f32,
}

#[repr(vec2)]
struct Uv {
    s: f32,
    t: f32,
}

fn main_image(frag_color: &mut Vec4, frag_coord: Vec2) {
    let uv = Uv { s: frag_coord.x , t: frag_coord.y  };
    let color = Color { r: uv.s, g: uv.t, b: 1.0};
    *frag_color = vec4(color.r, color.g, color.b, 1.0);
}
