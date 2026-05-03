type Color = Vec4;
type Pos = Vec2;

fn main_image(frag_coord: Pos, resolution: Pos, time: f32) -> Color {
    let uv = frag_coord / resolution;
    vec4(uv.x, uv.y, sin(time), 1.0)
}
