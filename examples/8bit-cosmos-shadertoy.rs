fn rect(p: Point, center: Point, size: Point) -> f32 {
    let d = abs(p - center) - size;
    1.0 - step(0.0, max(d.x, d.y))
}

fn circle(p: Point, center: Point, radius: f32) -> f32 {
    1.0 - step(radius, length(p - center))
}

fn ring(p: Point, center: Point, r1: f32, r2: f32) -> f32 {
    let outer = circle(p, center, r1);
    let inner = circle(p, center, r2);
    abs(outer - inner)
}

fn hash21(p: Point) -> f32 {
    fract(sin(dot(p, Point { x: 127.1, y: 311.7 })) * 43758.5453)
}

fn pixel(frag_coord: Point, resolution: Point, time: f32) -> Color {
    let screen = Point {
        x: frag_coord.x / resolution.x,
        y: frag_coord.y / resolution.y,
    };

    // 8bit低解像度化
    let pix = Point {
        x: floor(screen.x * 160.0) / 160.0,
        y: floor(screen.y * 90.0) / 90.0,
    };

    let uv = Point {
        x: pix.x * 2.0 - 1.0,
        y: pix.y * 2.0 - 1.0,
    };

    let t = floor(time * 8.0) / 8.0;

    // 星: 低解像度グリッド上で点滅
    let star_cell = floor(Point {
        x: pix.x * 96.0,
        y: pix.y * 54.0,
    });

    let star_rand = hash21(star_cell);
    let twinkle = step(0.5, fract(star_rand * 7.0 + t * 0.7));
    let stars = step(0.965, star_rand) * twinkle;

    // 遠景の小さい星層
    let star_cell2 = floor(Point {
        x: pix.x * 48.0 + 11.0,
        y: pix.y * 27.0 + 3.0,
    });

    let star_rand2 = hash21(star_cell2);
    let stars2 = step(0.94, star_rand2) * 0.55;

    // 惑星
    let planet_center = Point {
        x: -0.45 + 0.04 * sin(t * 0.8),
        y: 0.18 + 0.03 * cos(t * 0.6),
    };

    let planet = circle(uv, planet_center, 0.24);
    let planet_cut = circle(
        uv,
        Point {
            x: planet_center.x + 0.10,
            y: planet_center.y + 0.08,
        },
        0.25,
    );

    let crescent = planet * (1.0 - planet_cut);

    // 土星リング風
    let ring_shape = ring(
        Point {
            x: uv.x,
            y: uv.y * 2.8,
        },
        Point {
            x: planet_center.x,
            y: planet_center.y * 2.8,
        },
        0.36,
        0.30,
    );

    let ring_mask = 1.0 - circle(
        uv,
        Point {
            x: planet_center.x,
            y: planet_center.y,
        },
        0.18,
    );

    let saturn_ring = ring_shape * ring_mask;

    // 流星
    let comet_x = 1.4 - fract(time * 0.18) * 2.8;
    let comet_y = 0.68 - fract(time * 0.18) * 0.9;

    let comet_head = circle(
        uv,
        Point {
            x: comet_x,
            y: comet_y,
        },
        0.035,
    );

    let comet_tail = rect(
        uv,
        Point {
            x: comet_x + 0.15,
            y: comet_y + 0.05,
        },
        Point {
            x: 0.18,
            y: 0.015,
        },
    );

    // ロケット本体
    let ship_center = Point {
        x: 0.42,
        y: -0.35 + 0.08 * sin(t * 2.0),
    };

    let ship_body = rect(
        uv,
        ship_center,
        Point { x: 0.06, y: 0.16 },
    );

    let ship_nose = rect(
        uv,
        Point {
            x: ship_center.x,
            y: ship_center.y + 0.18,
        },
        Point { x: 0.035, y: 0.035 },
    );

    let ship_window = circle(
        uv,
        Point {
            x: ship_center.x,
            y: ship_center.y + 0.05,
        },
        0.035,
    );

    let fin_l = rect(
        uv,
        Point {
            x: ship_center.x - 0.07,
            y: ship_center.y - 0.11,
        },
        Point { x: 0.035, y: 0.06 },
    );

    let fin_r = rect(
        uv,
        Point {
            x: ship_center.x + 0.07,
            y: ship_center.y - 0.11,
        },
        Point { x: 0.035, y: 0.06 },
    );

    let flame = rect(
        uv,
        Point {
            x: ship_center.x,
            y: ship_center.y - 0.23,
        },
        Point {
            x: 0.035 + 0.02 * step(0.5, fract(time * 12.0)),
            y: 0.07,
        },
    );

    let ship = ship_body + ship_nose + ship_window + fin_l + fin_r + flame;

    // 8bit HUDっぽい十字
    let cross = rect(
        uv,
        Point { x: -0.78, y: -0.72 },
        Point { x: 0.18, y: 0.015 },
    ) + rect(
        uv,
        Point { x: -0.78, y: -0.72 },
        Point { x: 0.015, y: 0.18 },
    );

    // 走査線
    let scan = 0.80 + 0.20 * step(0.5, fract(frag_coord.y * 0.5));

    let mut v = 0.0;
    v = max(v, stars);
    v = max(v, stars2);
    v = max(v, crescent);
    v = max(v, saturn_ring);
    v = max(v, comet_head);
    v = max(v, comet_tail);
    v = max(v, ship);
    v = max(v, cross * 0.8);

    v = v * scan;

    // 完全白黒化
    v = step(0.42, v);

    Color { r: v, g: v, b: v }
}

// これ以下はadapter
#[repr(vec2)]
struct Point {
    x: f32,
    y: f32,
}
#[repr(vec3)]
struct Color {
    #[component(0)]
    r: f32,
    #[component(1)]
    g: f32,
    #[component(2)]
    b: f32,
}

#[builtin(iResolution)]
static i_resolution: Vec3 = vec3();

#[builtin(iTime)]
static i_time: f32 = 0;
fn mainImage(frag_color: &mut Vec4, frag_coord: Vec2) {
    let resolution = Point {
        x: i_resolution.x,
        y: i_resolution.y,
    };

    let col = pixel(
        Point {
            x: frag_coord.x,
            y: frag_coord.y,
        },
        resolution,
        i_time,
    );

    *frag_color = vec4(col.r, col.g, col.b, 1.0);
}

