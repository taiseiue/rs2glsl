fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let uv = frag_coord / resolution;
    let c = 0.5 + 0.5 * sin(time + uv.x * 10.0);
    vec4(c, uv.y, 1.0 - c, 1.0)
}
