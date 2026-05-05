const PI: f32 = 3.14159265359;
const TAU: f32 = 6.28318530718;

#[structlayout(vec2)]
struct Point {
    x: f32,
    y: f32,
}

#[structlayout(vec3)]
struct Color {
    r: f32,
    g: f32,
    b: f32,
}

fn point(x: f32, y: f32) -> Point {
    Point { x, y }
}

fn color(r: f32, g: f32, b: f32) -> Color {
    Color { r, g, b }
}

fn uv_from_frag(frag_coord: Point, resolution: Point) -> Point {
    Point {
        x: frag_coord.x / resolution.x,
        y: frag_coord.y / resolution.y,
    }
}

fn aspect_uv(uv: Point, resolution: Point) -> Point {
    Point {
        x: (uv.x - 0.5) * resolution.x / resolution.y,
        y: uv.y - 0.5,
    }
}

fn pixelize(uv: Point, resolution: Point, size: f32) -> Point {
    Point {
        x: floor(uv.x * resolution.x / size) * size / resolution.x,
        y: floor(uv.y * resolution.y / size) * size / resolution.y,
    }
}

fn repeat2(p: Point, cell: f32) -> Point {
    Point {
        x: mod_(p.x, cell) - cell * 0.5,
        y: mod_(p.y, cell) - cell * 0.5,
    }
}

fn hash11(x: f32) -> f32 {
    fract(sin(x * 127.1) * 43758.5453123)
}

fn hash21(p: Point) -> f32 {
    fract(sin(p.x * 127.1 + p.y * 311.7) * 43758.5453123)
}

fn hash22(p: Point) -> Point {
    Point {
        x: hash21(p),
        y: hash21(Point {
            x: p.x + 19.19,
            y: p.y + 47.77,
        }),
    }
}


fn rotate(p: Point, angle: f32) -> Point {
    let c = cos(angle);
    let s = sin(angle);

    Point {
        x: p.x * c - p.y * s,
        y: p.x * s + p.y * c,
    }
}

fn circle(p: Point, radius: f32) -> f32 {
    length(Point { x: p.x, y: p.y }) - radius
}

fn box2(p: Point, size: Point) -> f32 {
    let q = Point {
        x: abs(p.x) - size.x,
        y: abs(p.y) - size.y,
    };

    length(Point {
        x: max(q.x, 0.0),
        y: max(q.y, 0.0),
    }) + min(max(q.x, q.y), 0.0)
}

fn palette(t: f32) -> Color {
    Color {
        r: 0.5 + 0.5 * cos(TAU * (t + 0.00)),
        g: 0.5 + 0.5 * cos(TAU * (t + 0.33)),
        b: 0.5 + 0.5 * cos(TAU * (t + 0.67)),
    }
}

fn clampf(x: f32, lo: f32, hi: f32) -> f32 {
    min(max(x, lo), hi)
}

fn add_color(a: Color, b: Color) -> Color {
    color(a.r + b.r, a.g + b.g, a.b + b.b)
}

fn mul_color(c: Color, k: f32) -> Color {
    color(c.r * k, c.g * k, c.b * k)
}

fn mix_color(a: Color, b: Color, t: f32) -> Color {
    color(
        mix(a.r, b.r, t),
        mix(a.g, b.g, t),
        mix(a.b, b.b, t),
    )
}
