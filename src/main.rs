use minifb::{Key, MouseButton, MouseMode, Window, WindowOptions};
use rayon::prelude::*;

const WIDTH: usize = 1200;
const HEIGHT: usize = 900;
const WAVE_SPEED: f32 = 60.0;
const AMPLITUDE: f32 = 0.70;
const DECAY: f32 = 0.009;
const DEFAULT_FREQUENCY: f32 = 2.0;
const FREQ_RATE: f32 = 1.0;                       // Hz/s
const PHASE_RATE: f32 = std::f32::consts::PI;     // rad/s

#[derive(Clone, Copy)]
enum Selected {
    Point(usize),
    Line(usize),
}

struct PointSource {
    x: f32,
    y: f32,
    age: f32,
    frequency: f32,
    phase_offset: f32,
    muted: bool,
}

struct LineSource {
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    age: f32,
    frequency: f32,
    phase_offset: f32,
    muted: bool,
}

fn dist_to_segment(px: f32, py: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len_sq = dx * dx + dy * dy;
    if len_sq == 0.0 {
        return ((px - x1).powi(2) + (py - y1).powi(2)).sqrt();
    }
    let t = ((px - x1) * dx + (py - y1) * dy) / len_sq;
    let t = t.clamp(0.0, 1.0);
    let nx = x1 + t * dx;
    let ny = y1 + t * dy;
    ((px - nx).powi(2) + (py - ny).powi(2)).sqrt()
}

fn draw_circle(buffer: &mut [u32], cx: i32, cy: i32, r: i32, color: u32) {
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy <= r * r {
                let x = cx + dx;
                let y = cy + dy;
                if x >= 0 && x < WIDTH as i32 && y >= 0 && y < HEIGHT as i32 {
                    buffer[y as usize * WIDTH + x as usize] = color;
                }
            }
        }
    }
}

fn draw_line_pixels(buffer: &mut [u32], x1: i32, y1: i32, x2: i32, y2: i32, color: u32) {
    let dx = (x2 - x1).abs();
    let dy = -(y2 - y1).abs();
    let sx = if x1 < x2 { 1 } else { -1 };
    let sy = if y1 < y2 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut x = x1;
    let mut y = y1;
    loop {
        if x >= 0 && x < WIDTH as i32 && y >= 0 && y < HEIGHT as i32 {
            buffer[y as usize * WIDTH + x as usize] = color;
        }
        if x == x2 && y == y2 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy { err += dy; x += sx; }
        if e2 <= dx { err += dx; y += sy; }
    }
}

fn main() {
    let mut window = Window::new(
        "Fale  |  LPM: punktowe  |  PPM: liniowe  |  kliknij zrodlo i uzywaj strzalek",
        WIDTH,
        HEIGHT,
        WindowOptions::default(),
    )
    .expect("Nie mozna otworzyc okna");

    window.set_target_fps(60);

    let mut buffer: Vec<u32> = vec![0; WIDTH * HEIGHT];
    let mut point_sources: Vec<PointSource> = Vec::new();
    let mut line_sources: Vec<LineSource> = Vec::new();
    let mut selected: Option<Selected> = None;
    let mut lmb_was_down = false;
    let mut rmb_was_down = false;
    let mut rmb_start: Option<(f32, f32)> = None;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let dt = 1.0 / 60.0_f32;

        let lmb_down = window.get_mouse_down(MouseButton::Left);
        let rmb_down = window.get_mouse_down(MouseButton::Right);
        let mouse_pos = window.get_mouse_pos(MouseMode::Clamp);
        let keys = window.get_keys();

        // --- Strzalki: zmiana czestotliwosci i fazy wybranego zrodla ---
        let freq_delta = if keys.contains(&Key::Up) { FREQ_RATE * dt }
                         else if keys.contains(&Key::Down) { -FREQ_RATE * dt }
                         else { 0.0 };
        let phase_delta = if keys.contains(&Key::Right) { PHASE_RATE * dt }
                          else if keys.contains(&Key::Left) { -PHASE_RATE * dt }
                          else { 0.0 };

        match selected {
            Some(Selected::Point(i)) => {
                let src = &mut point_sources[i];
                src.frequency = (src.frequency + freq_delta).max(0.1);
                src.phase_offset += phase_delta;
            }
            Some(Selected::Line(i)) => {
                let src = &mut line_sources[i];
                src.frequency = (src.frequency + freq_delta).max(0.1);
                src.phase_offset += phase_delta;
            }
            None => {}
        }

        // --- Delete: usun wybrane zrodlo ---
        if window.is_key_pressed(Key::Delete, minifb::KeyRepeat::No) {
            match selected {
                Some(Selected::Point(i)) => { point_sources.remove(i); selected = None; }
                Some(Selected::Line(i))  => { line_sources.remove(i);  selected = None; }
                None => {}
            }
        }

        // --- M: wycisz / odcisz wybrane zrodlo ---
        if window.is_key_pressed(Key::M, minifb::KeyRepeat::No) {
            match selected {
                Some(Selected::Point(i)) => { point_sources[i].muted ^= true; }
                Some(Selected::Line(i))  => { line_sources[i].muted  ^= true; }
                None => {}
            }
        }

        // --- C: usun wszystkie zrodla ---
        if window.is_key_pressed(Key::C, minifb::KeyRepeat::No) {
            point_sources.clear();
            line_sources.clear();
            selected = None;
        }

        // --- S: zapisz aktualny kadr do PNG ---
        if window.is_key_pressed(Key::S, minifb::KeyRepeat::No) {
            let rgb: Vec<u8> = buffer.iter().flat_map(|&p| {
                [((p >> 16) & 0xFF) as u8, ((p >> 8) & 0xFF) as u8, (p & 0xFF) as u8]
            }).collect();
            let _ = image::save_buffer(
                "fale.png", &rgb, WIDTH as u32, HEIGHT as u32, image::ColorType::Rgb8,
            );
        }

        // --- Lewy przycisk: wybierz istniejace lub stworz nowe zrodlo punktowe ---
        if lmb_down && !lmb_was_down {
            if let Some((mx, my)) = mouse_pos {
                let hit_point = point_sources.iter().enumerate().find(|(_, s)| {
                    ((mx - s.x).powi(2) + (my - s.y).powi(2)).sqrt() < 12.0
                }).map(|(i, _)| Selected::Point(i));

                let hit = hit_point.or_else(|| {
                    line_sources.iter().enumerate().find(|(_, s)| {
                        dist_to_segment(mx, my, s.x1, s.y1, s.x2, s.y2) < 12.0
                    }).map(|(i, _)| Selected::Line(i))
                });

                if let Some(sel) = hit {
                    selected = Some(sel);
                } else {
                    point_sources.push(PointSource {
                        x: mx, y: my, age: 0.0,
                        frequency: DEFAULT_FREQUENCY,
                        phase_offset: 0.0,
                        muted: false,
                    });
                    selected = Some(Selected::Point(point_sources.len() - 1));
                }
            }
        }
        lmb_was_down = lmb_down;

        // --- Prawy przycisk: zrodlo liniowe ---
        if rmb_down && !rmb_was_down {
            rmb_start = mouse_pos;
        }
        if !rmb_down && rmb_was_down {
            if let (Some((sx, sy)), Some((ex, ey))) = (rmb_start.take(), mouse_pos) {
                line_sources.push(LineSource {
                    x1: sx, y1: sy, x2: ex, y2: ey, age: 0.0,
                    frequency: DEFAULT_FREQUENCY,
                    phase_offset: 0.0,
                    muted: false,
                });
                selected = Some(Selected::Line(line_sources.len() - 1));
            }
        }
        rmb_was_down = rmb_down;

        // --- Aktualizacja wieku ---
        for src in &mut point_sources { src.age += dt; }
        for src in &mut line_sources  { src.age += dt; }

        // --- Renderowanie fal (wielowatkowo: kazdy wiersz w osobnym watku) ---
        buffer.par_chunks_mut(WIDTH).enumerate().for_each(|(py, row)| {
            for (px, pixel) in row.iter_mut().enumerate() {
                let mut total: f32 = 0.0;

                for src in point_sources.iter().filter(|s| !s.muted) {
                    let dx = px as f32 - src.x;
                    let dy = py as f32 - src.y;
                    let dist = (dx * dx + dy * dy).sqrt();
                    let phase = src.age - dist / WAVE_SPEED;
                    if phase > 0.0 {
                        let decay = (-DECAY * dist).exp();
                        total += AMPLITUDE * decay
                            * (2.0 * std::f32::consts::PI * src.frequency * phase
                               + src.phase_offset).sin();
                    }
                }

                for src in line_sources.iter().filter(|s| !s.muted) {
                    let dist = dist_to_segment(
                        px as f32, py as f32,
                        src.x1, src.y1, src.x2, src.y2,
                    );
                    let phase = src.age - dist / WAVE_SPEED;
                    if phase > 0.0 {
                        let decay = (-DECAY * dist).exp();
                        total += AMPLITUDE * decay
                            * (2.0 * std::f32::consts::PI * src.frequency * phase
                               + src.phase_offset).sin();
                    }
                }

                let normalized = (total.tanh() + 1.0) / 2.0;
                *pixel = value_to_color(normalized);
            }
        });

        // --- Markery zrodel ---
        for (i, src) in point_sources.iter().enumerate() {
            let is_sel = matches!(selected, Some(Selected::Point(j)) if j == i);
            let color = if is_sel { 0xFFFF00 } else if src.muted { 0xFF4400 } else { 0x888888 };
            draw_circle(&mut buffer, src.x as i32, src.y as i32, 5, color);
        }
        for (i, src) in line_sources.iter().enumerate() {
            let is_sel = matches!(selected, Some(Selected::Line(j)) if j == i);
            let color = if is_sel { 0xFFFF00 } else if src.muted { 0xFF4400 } else { 0x888888 };
            draw_line_pixels(&mut buffer,
                src.x1 as i32, src.y1 as i32, src.x2 as i32, src.y2 as i32, color);
            draw_circle(&mut buffer, src.x1 as i32, src.y1 as i32, 4, color);
            draw_circle(&mut buffer, src.x2 as i32, src.y2 as i32, 4, color);
        }

        // --- Podglad linii podczas przeciagania ---
        if rmb_down {
            if let (Some((sx, sy)), Some((ex, ey))) = (rmb_start, mouse_pos) {
                draw_line_pixels(&mut buffer,
                    sx as i32, sy as i32, ex as i32, ey as i32, 0xFFFFFF);
            }
        }

        // --- Tytul okna z parametrami wybranego zrodla ---
        let title = match selected {
            Some(Selected::Point(i)) => {
                let s = &point_sources[i];
                let mute_tag = if s.muted { "  [WYCISZONE]" } else { "" };
                format!("Fale  |  [Punkt #{}]{mute_tag}  czest: {:.2} Hz  faza: {:.2} rad  \
                         |  gora/dol: czestotliwosc  lewo/prawo: faza  M: wycisz  S: PNG  C: wyczysc",
                         i + 1, s.frequency, s.phase_offset)
            }
            Some(Selected::Line(i)) => {
                let s = &line_sources[i];
                let mute_tag = if s.muted { "  [WYCISZONE]" } else { "" };
                format!("Fale  |  [Linia #{}]{mute_tag}  czest: {:.2} Hz  faza: {:.2} rad  \
                         |  gora/dol: czestotliwosc  lewo/prawo: faza  M: wycisz  S: PNG  C: wyczysc",
                         i + 1, s.frequency, s.phase_offset)
            }
            None => "Fale  |  LPM: punktowe  |  PPM: liniowe  \
                     |  kliknij zrodlo  |  S: PNG  C: wyczysc".to_string(),
        };
        window.set_title(&title);

        window.update_with_buffer(&buffer, WIDTH, HEIGHT).unwrap();
    }
}

fn value_to_color(t: f32) -> u32 {
    // t=0.5 → black (zero amplitude / background)
    // t→0   → blue  (negative peaks)
    // t→1   → yellow (positive peaks)
    let (r, g, b) = if t < 0.5 {
        let s = 1.0 - t / 0.5;
        (0.0_f32, 0.0, s)
    } else {
        let s = (t - 0.5) / 0.5;
        (s, s, 0.0_f32)
    };
    ((r * 255.0) as u32) << 16 | ((g * 255.0) as u32) << 8 | (b * 255.0) as u32
}
