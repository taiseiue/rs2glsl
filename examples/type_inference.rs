use shader_prelude::*;

fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let uv = frag_coord / resolution;
    let t = time * 0.5;
    let col = Vec3::new(uv.x, uv.y, sin(t));
    let n = normalize(col);
    let brightness = dot(n, Vec3::new(0.577, 0.577, 0.577));
    Vec4::new(n.x, n.y, n.z, brightness)
}
