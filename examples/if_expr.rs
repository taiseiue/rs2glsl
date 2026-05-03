fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let uv = frag_coord / resolution;

    let col = if uv.x > 0.5 {
        vec3(1.0, 0.2, 0.2)
    } else {
        vec3(0.2, 0.2, 1.0)
    };

    let edge = if uv.x < 0.1 {
        1.0
    } else if uv.x > 0.9 {
        1.0
    } else {
        0.0
    };

    vec4(col.x, col.y, col.z, edge)
}
