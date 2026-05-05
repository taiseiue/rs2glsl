fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let uv = frag_coord / resolution;
    let flipped = -uv;
    let inv_time = -time;
    let inside = uv.x > 0.1;
    let outside = !inside;
    let col = if outside {
        vec3(flipped.x + 1.0, flipped.y + 1.0, sin(inv_time))
    } else {
        vec3(uv.x, uv.y, 0.5)
    };
    vec4(col.x, col.y, col.z, 1.0)
}
