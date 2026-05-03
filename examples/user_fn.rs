fn palette(t: f32) -> Vec3 {
    vec3(sin(t), sin(t + 2.0), sin(t + 4.0)) * 0.5 + 0.5
}

fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let uv = frag_coord / resolution;
    let color = palette(uv.x + time);
    vec4(color.x, color.y, color.z, 1.0)
}
