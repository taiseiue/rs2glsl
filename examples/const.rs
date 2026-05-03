const PI: f32 = 3.14159;
const TWO_PI: f32 = PI * 2.0;
const LIGHT_DIR: Vec3 = vec3(0.577, 0.577, 0.577);

fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let uv = frag_coord / resolution;
    let angle = TWO_PI * uv.x + time;
    let n = normalize(vec3(cos(angle), sin(angle), 1.0));
    let diffuse = dot(n, LIGHT_DIR);
    vec4(diffuse, diffuse, diffuse, 1.0)
}
