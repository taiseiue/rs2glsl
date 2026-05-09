use std::ops::{Add,Sub};
use rs2glsl_macros::structlayout;
use rs2glsl_prelude::*;

pub const PI: f32 = 3.14159265359;
pub const TAU: f32 = 6.28318530718;

#[structlayout(vec2)]
pub struct Point {
    x: f32,
    y: f32,
}

#[structlayout(vec3)]
pub struct Color {
    r: f32,
    g: f32,
    b: f32,
}

impl Add for Point {
    type Output = Point;

    fn add(self, rhs: Point) -> Point {
        Point {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub for Point {
    type Output = Point;

    fn sub(self, rhs: Point) -> Point {
        Point {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl Add for Color {
    type Output = Color;

    fn add(self, rhs: Color) -> Color {
        Color {
            r: self.r + rhs.r,
            g: self.g + rhs.g,
            b: self.b + rhs.b,
        }
    }
}

impl Sub for Color {
    type Output = Color;

    fn sub(self, rhs: Color) -> Color {
        Color {
            r: self.r - rhs.r,
            g: self.g - rhs.g,
            b: self.b - rhs.b,
        }
    }
}

pub fn uv_from_frag(frag_coord: Point, resolution: Point) -> Point {
    Point {
        x: frag_coord.x / resolution.x,
        y: frag_coord.y / resolution.y,
    }
}

pub fn aspect_uv(uv: Point, resolution: Point) -> Point {
    Point {
        x: (uv.x - 0.5) * resolution.x / resolution.y,
        y: uv.y - 0.5,
    }
}

pub fn pixelize(uv: Point, resolution: Point, size: f32) -> Point {
    Point {
        x: floor(uv.x * resolution.x / size) * size / resolution.x,
        y: floor(uv.y * resolution.y / size) * size / resolution.y,
    }
}

pub fn repeat2(p: Point, cell: f32) -> Point {
    Point {
        x: mod_(p.x, cell) - cell * 0.5,
        y: mod_(p.y, cell) - cell * 0.5,
    }
}

pub fn hash11(x: f32) -> f32 {
    fract(sin(x * 127.1) * 43758.5453123)
}

pub fn hash21(p: Point) -> f32 {
    fract(sin(p.x * 127.1 + p.y * 311.7) * 43758.5453123)
}

pub fn hash22(p: Point) -> Point {
    Point {
        x: hash21(p),
        y: hash21(Point {
            x: p.x + 19.19,
            y: p.y + 47.77,
        }),
    }
}

pub fn rotate(p: Point, angle: f32) -> Point {
    let c = cos(angle);
    let s = sin(angle);

    Point {
        x: p.x * c - p.y * s,
        y: p.x * s + p.y * c,
    }
}

pub fn circle(p: Point, radius: f32) -> f32 {
    length(Point { x: p.x, y: p.y }) - radius
}

pub fn box2(p: Point, size: Point) -> f32 {
    let q = Point {
        x: abs(p.x) - size.x,
        y: abs(p.y) - size.y,
    };

    length(Point {
        x: max(q.x, 0.0),
        y: max(q.y, 0.0),
    }) + min(max(q.x, q.y), 0.0)
}

pub fn palette(t: f32) -> Color {
    Color {
        r: 0.5 + 0.5 * cos(TAU * (t + 0.00)),
        g: 0.5 + 0.5 * cos(TAU * (t + 0.33)),
        b: 0.5 + 0.5 * cos(TAU * (t + 0.67)),
    }
}

pub fn clampf(x: f32, lo: f32, hi: f32) -> f32 {
    min(max(x, lo), hi)
}

pub fn mul_color(c: Color, k: f32) -> Color {
    Color { r: c.r * k, g: c.g * k, b: c.b * k }
}

pub fn mix_color(a: Color, b: Color, t: f32) -> Color {
    Color { r: mix(a.r, b.r, t), g: mix(a.g, b.g, t), b: mix(a.b, b.b, t) }
}
