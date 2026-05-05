fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let uv = frag_coord / resolution;
    let col = vec4(uv.x, uv.y, 0.0, 1.0);

    for i in 0..3 {
        col.z = col.z + 0.2;
    }

    col
}
