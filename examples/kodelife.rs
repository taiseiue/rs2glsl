fn pixel(
    uv: Point,
    resolution: Point,
    time: f32,
    mouse: Point,
    spectrum: Color,
) -> Color {
    Color {
        r: abs(sin(cos(time + 3.0 * uv.y) * 2.0 * uv.x + time)),
        g: abs(cos(sin(time + 2.0 * uv.x) * 3.0 * uv.y + time)),
        b: spectrum.r * 100.0,
    }
}

#[builtin(time)]
static time: f32 = 0.0;

#[builtin(resolution)]
static resolution: Vec2 = vec2();

#[builtin(mouse)]
static mouse: Vec2 = vec2();

#[builtin(spectrum)]
static spectrum: Vec3 = vec3();


// KodeLife 側で inData.v_texcoord を builtin として読む想定
#[builtin(inData.v_texcoord)]
static v_texcoord: Vec2 = vec2();

fn main(frag_color: &mut Vec4) {
    let uv = Point {
        x: -1.0 + 2.0 * v_texcoord.x,
        y: -1.0 + 2.0 * v_texcoord.y,
    };

    let col = pixel(
        uv,
        Point {
            x: resolution.x,
            y: resolution.y,
        },
        time,
        Point {
            x: mouse.x,
            y: mouse.y,
        },
        Color {
            r: spectrum.x,
            g: spectrum.y,
            b: spectrum.z,
        },
    );

    *frag_color = vec4(col.r, col.g, col.b, 1.0);
}
#[repr(vec2)]
struct Point {
    x: f32,
    y: f32,
}

#[repr(vec3)]
struct Color {
    r: f32,
    g: f32,
    b: f32,
}
