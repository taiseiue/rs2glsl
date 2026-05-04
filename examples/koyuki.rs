const PIXEL_SIZE: f32 = 50.0;

fn stepf(edge: f32, x: f32) -> f32 {
    if x < edge { 0.0 } else { 1.0 }
}

fn dot_at(puv: Point, target: Point) -> f32 {
    let d = point(
        abs(puv.x - target.x) * PIXEL_SIZE,
        abs(puv.y - target.y) * PIXEL_SIZE,
    );

    stepf(d.x, 0.5) * stepf(d.y, 0.5)
}

fn block_at(puv: Point, center: Point, radius: f32) -> f32 {
    let d = point(
        abs(puv.x - center.x) * PIXEL_SIZE,
        abs(puv.y - center.y) * PIXEL_SIZE,
    );

    stepf(max(d.x, d.y), radius)
}

fn scanner(puv: Point, t: f32) -> f32 {
    let scan_y = mod_(t * 0.4, 1.5) - 0.25;
    let d = abs(puv.y - (scan_y * 2.0 - 1.0));
    stepf(d, 1.0 / PIXEL_SIZE)
}

fn audio_bars(puv: Point, bass_amp: f32, mid_amp: f32, high_amp: f32, t: f32) -> f32 {
    let mut result = 0.0;
    let bar_count = 32;

    for i in 0..32 {
        let fi = i as f32;
        let fn_ = bar_count as f32;

        let bar_x = (fi / fn_ - 0.5) * 1.6 + (0.8 / fn_);
        let freq = fi / fn_;

        let mut h = if freq < 0.33 {
            bass_amp
        } else if freq < 0.66 {
            mid_amp
        } else {
            high_amp
        };

        h *= 0.6 + 0.4 * abs(sin(fi * 0.7 + t * 4.0));
        h = pow(h, 0.7);

        let bar_w = 1.5 / fn_;

        if abs(puv.x - bar_x) < bar_w * 0.5 {
            if abs(puv.y) < h * 0.4 {
                result = 1.0;
            }
        }
    }

    result
}

fn moving_dots(puv: Point, t: f32, bass_amp: f32) -> f32 {
    let mut result = 0.0;

    for i in 0..12 {
        let fi = i as f32;

        let speed = 0.3 + fi * 0.07;
        let radius_x = 0.6 + sin(fi) * 0.15;
        let radius_y = 0.4 + cos(fi * 1.3) * 0.15;
        let phase = fi * 0.524;

        let pos = point(
            sin(t * speed + phase) * radius_x,
            cos(t * speed * 1.3 + phase * 1.7) * radius_y,
        );

        let size = 0.6 + bass_amp * 1.5;
        result = max(result, block_at(puv, pos, size));
    }

    result
}

fn grid(puv: Point, t: f32) -> f32 {
    let grid_uv = point(puv.x * 6.0, puv.y * 6.0);
    let cell = point(floor(grid_uv.x), floor(grid_uv.y));
    let frac = point(fract(grid_uv.x), fract(grid_uv.y));

    let d = point(abs(frac.x - 0.5) * 2.0, abs(frac.y - 0.5) * 2.0);

    let dot = stepf(max(d.x, d.y), 0.05);

    let h = hash21(cell);
    let blink = stepf(0.95, fract(h + t * 0.3));

    dot * (0.3 + blink * 0.7)
}

fn crosshair(puv: Point, t: f32) -> f32 {
    let mut result = 0.0;

    let line_thick = 1.0 / PIXEL_SIZE;
    let cross_h = stepf(abs(puv.y), line_thick * 0.5) * stepf(abs(puv.x), 0.08);
    let cross_v = stepf(abs(puv.x), line_thick * 0.5) * stepf(abs(puv.y), 0.08);

    result = max(result, cross_h);
    result = max(result, cross_v);
    result = max(result, block_at(puv, point(0.0, 0.0), 1.0));

    let pulse = 0.5 + 0.5 * sin(t * 2.0);
    let corner = 0.06 + pulse * 0.01;

    result = max(result, block_at(puv, point(corner, corner), 0.6));
    result = max(result, block_at(puv, point(-corner, corner), 0.6));
    result = max(result, block_at(puv, point(corner, -corner), 0.6));
    result = max(result, block_at(puv, point(-corner, -corner), 0.6));

    result
}

fn glitch(puv: Point, t: f32, bass_amp: f32) -> Point {
    let line_seed = floor(puv.y * 30.0 + t * 5.0);
    let r = hash11(line_seed);

    let mut out_p = puv;

    if r > 1.0 - bass_amp * 0.8 {
        let shift = (hash11(line_seed + 7.3) - 0.5) * bass_amp * 0.3;
        out_p.x += shift;
    }

    out_p
}

fn pixel(frag_coord: Point, resolution: Point, spectrum: Color, time: f32) -> Color {
    let uv_raw = point(
        (frag_coord.x - resolution.x * 0.5) / resolution.y,
        (frag_coord.y - resolution.y * 0.5) / resolution.y,
    );

    let bass = spectrum.r;
    let mid = spectrum.g;
    let high = spectrum.b;

    let bass_strong = pow(bass, 1.3);

    let uv = glitch(uv_raw, time, bass_strong);
    let puv = point(
        floor(uv.x * PIXEL_SIZE) / PIXEL_SIZE,
        floor(uv.y * PIXEL_SIZE) / PIXEL_SIZE,
    );

    let mut px = 0.0;

    let bg = grid(puv, time);
    px = max(px, micro_equalizer(puv, bass, mid, high, time));
    px = max(px, moving_dots(puv, time, bass_strong));
    px = max(px, crosshair(puv, time));
    px = max(px, scanner(puv, time));

    let mut col = color(0.0, 0.0, 0.0);

    col = add_color(col, mul_color(color(0.12, 0.12, 0.12), bg));
    col = add_color(col, mul_color(color(1.0, 1.0, 1.0), px));

    if bass_strong > 0.4 {
        let seed = point(
            puv.x * 100.0 + floor(time * 8.0),
            puv.y * 100.0 + floor(time * 8.0),
        );
        let red_flash = stepf(0.7, hash21(seed));
        let amount = px * red_flash * (bass_strong - 0.4) * 1.5;
        col = mix_color(col, color(1.0, 0.2, 0.2), amount);
    }

    col = mul_color(col, 1.0 + high * 0.5);

    let scan = 0.85 + 0.15 * sin(frag_coord.y * 1.8);
    col = mul_color(col, scan);

    let pix_gap = point(fract(uv.x * PIXEL_SIZE), fract(uv.y * PIXEL_SIZE));
    let gap = stepf(0.05, pix_gap.x) * stepf(0.05, pix_gap.y);
    col = mul_color(col, 0.85 + 0.15 * gap);

    let vig_len = length(point(uv_raw.x * 0.9, uv_raw.y));
    let vig = 1.0 - smoothstep(0.5, 1.4, vig_len);
    col = mul_color(col, vig);

    let ca = bass_strong * 0.005;
    let ca_wave = sin(uv_raw.x * 10.0);
    col.r *= 1.0 + ca * ca_wave;
    col.b *= 1.0 - ca * ca_wave;

    let grain = (hash21(point(frag_coord.x + time * 7.0, frag_coord.y + time * 7.0)) - 0.5) * 0.04;

    col = add_color(col, color(grain, grain, grain));

    col = mix_color(col, color(col.r * 0.95, col.g * 1.02, col.b * 0.92), 0.15);

    color(
        clampf(col.r, 0.0, 1.0),
        clampf(col.g, 0.0, 1.0),
        clampf(col.b, 0.0, 1.0),
    )
}

fn micro_equalizer(puv: Point, bass: f32, mid: f32, high: f32, t: f32) -> f32 {
    let mut result = 0.0;

    for i in 0..40 {
        let fi = i as f32;
        let n = 40.0;

        let x = (fi / n - 0.5) * 1.7;
        let freq = fi / n;

        let amp = if freq < 0.33 {
            bass
        } else if freq < 0.66 {
            mid
        } else {
            high
        };

        let h = pow(amp, 0.75) * 0.35;
        let y_count = floor(h * 18.0);

        for j in 0..18 {
            let fj = j as f32;
            let y = (fj / 18.0) * 0.7 - 0.35;

            let activea = stepf(fj, y_count);
            let flicker = stepf(0.25, hash21(point(fi, fj)) + 0.25 * sin(t * 8.0 + fi));

            result = max(result, block_at(puv, point(x, y), 0.32) * activea * flicker);
        }
    }

    result
}

fn micro_sparks(puv: Point, t: f32, amp: f32) -> f32 {
    let mut result = 0.0;

    for i in 0..48 {
        let fi = i as f32;

        let seed = point(fi * 13.1, fi * 7.7);
        let h = hash22(seed);

        let base = point(
            h.x * 1.8 - 0.9,
            h.y * 1.2 - 0.6,
        );

        let wobble = point(
            sin(t * (0.7 + h.x * 2.0) + fi) * 0.03,
            cos(t * (0.6 + h.y * 2.0) + fi * 1.3) * 0.03,
        );

        let pos = point(base.x + wobble.x, base.y + wobble.y);

        let blink = stepf(0.55, fract(hash11(fi) + t * (0.4 + h.x)));
        let size = 0.35 + amp * 0.5;

        result = max(result, block_at(puv, pos, size) * blink);
    }

    result
}
