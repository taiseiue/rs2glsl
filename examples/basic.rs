mod std;
mod types;
use crate::std::*;
use crate::types::*;

fn pixel(frag_coord: Point, resolution: Point, spectrum: Color, time: f32) -> Color {
    let uv = Point {
        x: frag_coord.x / resolution.x,
        y: frag_coord.y / resolution.y,
    };

    Color {
        r: uv.x,
        g: uv.y,
        b: spectrum.r,
    }
}
