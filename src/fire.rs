//! The PSX Doom fire, see https://fabiensanglard.net/doom_fire_psx/

use std::fmt::Write;
use std::time::{SystemTime, UNIX_EPOCH};

pub const MAX_HEAT: u8 = 36;

const PALETTE: [(u8, u8, u8); MAX_HEAT as usize + 1] = [
    (0x07, 0x07, 0x07), (0x1f, 0x07, 0x07), (0x2f, 0x0f, 0x07), (0x47, 0x0f, 0x07),
    (0x57, 0x17, 0x07), (0x67, 0x1f, 0x07), (0x77, 0x1f, 0x07), (0x8f, 0x27, 0x07),
    (0x9f, 0x2f, 0x07), (0xaf, 0x3f, 0x07), (0xbf, 0x47, 0x07), (0xc7, 0x47, 0x07),
    (0xdf, 0x4f, 0x07), (0xdf, 0x57, 0x07), (0xdf, 0x57, 0x07), (0xd7, 0x5f, 0x07),
    (0xd7, 0x5f, 0x07), (0xd7, 0x67, 0x0f), (0xcf, 0x6f, 0x0f), (0xcf, 0x77, 0x0f),
    (0xcf, 0x7f, 0x0f), (0xcf, 0x87, 0x17), (0xc7, 0x87, 0x17), (0xc7, 0x8f, 0x17),
    (0xc7, 0x97, 0x1f), (0xbf, 0x9f, 0x1f), (0xbf, 0x9f, 0x1f), (0xbf, 0xa7, 0x27),
    (0xbf, 0xa7, 0x27), (0xbf, 0xaf, 0x2f), (0xb7, 0xaf, 0x2f), (0xb7, 0xb7, 0x2f),
    (0xb7, 0xb7, 0x37), (0xcf, 0xcf, 0x6f), (0xdf, 0xdf, 0x9f), (0xef, 0xef, 0xc7),
    (0xff, 0xff, 0xff),
];

pub struct Fire {
    width: usize,
    height: usize,
    heat: Vec<u8>,
    decay: Vec<f32>,
    rng: u32,
}

impl Fire {
    pub fn new(width: usize, height: usize) -> Fire {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(1, |d| d.subsec_nanos());
        Fire::seeded(width, height, nanos)
    }

    fn seeded(width: usize, height: usize, seed: u32) -> Fire {
        Fire {
            width,
            height,
            heat: vec![0; width * height],
            decay: vec![0.0; width],
            rng: seed | 1,
        }
    }

    // xorshift32, plenty for flames
    fn next_rand(&mut self) -> u32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        x
    }

    /// Advance one frame. `base` is the heat fed into the bottom row,
    /// `levels` are loads from 0.0 to 1.0 spread evenly across the width.
    pub fn step(&mut self, base: u8, levels: &[f32]) {
        let (w, h) = (self.width, self.height);
        if w == 0 || h < 2 || levels.is_empty() {
            return;
        }
        let base = base.min(MAX_HEAT);

        // A particle loses `decay` heat per row on average, so the flame
        // reaches about base / decay rows up.
        for x in 0..w {
            let reach = h as f32 * (0.08 + 0.87 * column_level(levels, x, w));
            self.decay[x] = base as f32 / reach.max(1.0);
        }
        self.heat[(h - 1) * w..].fill(base);

        for y in 1..h {
            for x in 0..w {
                let heat = self.heat[y * w + x];
                let r = self.next_rand();
                let to = (x + 1).saturating_sub((r & 3) as usize).min(w - 1);
                let decay = self.decay[x];
                let mut cool = decay as u32;
                if ((r >> 8) as f32 / (1 << 24) as f32) < decay.fract() {
                    cool += 1;
                }
                self.heat[(y - 1) * w + to] = heat.saturating_sub(cool.min(255) as u8);
            }
        }
    }

    /// Append the frame as escape codes, two pixels per cell with half blocks.
    /// Cold pixels keep the terminal's own background.
    pub fn draw(&self, out: &mut String) {
        let w = self.width;
        let mut fg = None;
        let mut bg = None;
        for row in 0..self.height / 2 {
            let _ = write!(out, "\x1b[{};1H", row + 1);
            for x in 0..w {
                let top = self.heat[2 * row * w + x];
                let bottom = self.heat[(2 * row + 1) * w + x];
                let (glyph, want_fg, want_bg) = match (top, bottom) {
                    (0, 0) => (' ', fg, None),
                    (t, 0) => ('▀', Some(t), None),
                    (0, b) => ('▄', Some(b), None),
                    (t, b) => ('▀', Some(t), Some(b)),
                };
                if want_bg != bg {
                    match want_bg {
                        Some(heat) => {
                            let (r, g, b) = PALETTE[heat as usize];
                            let _ = write!(out, "\x1b[48;2;{r};{g};{b}m");
                        }
                        None => out.push_str("\x1b[49m"),
                    }
                    bg = want_bg;
                }
                if want_fg != fg {
                    if let Some(heat) = want_fg {
                        let (r, g, b) = PALETTE[heat as usize];
                        let _ = write!(out, "\x1b[38;2;{r};{g};{b}m");
                    }
                    fg = want_fg;
                }
                out.push(glyph);
            }
        }
        out.push_str("\x1b[0m");
    }
}

// Load under column x. Neighbouring levels are blended so cores
// don't show up as hard stripes.
fn column_level(levels: &[f32], x: usize, width: usize) -> f32 {
    let n = levels.len();
    if n == 1 {
        return levels[0];
    }
    let pos = ((x as f32 + 0.5) / width as f32 * n as f32 - 0.5).clamp(0.0, (n - 1) as f32);
    let i = (pos as usize).min(n - 2);
    let t = pos - i as f32;
    let t = t * t * (3.0 - 2.0 * t);
    levels[i] + (levels[i + 1] - levels[i]) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flame_height(fire: &Fire) -> usize {
        let lit = fire.heat.chunks(fire.width).position(|row| row.iter().any(|&h| h > 0));
        fire.height - lit.unwrap_or(fire.height)
    }

    #[test]
    fn more_load_burns_higher() {
        let mut idle = Fire::seeded(40, 60, 7);
        let mut busy = Fire::seeded(40, 60, 7);
        for _ in 0..200 {
            idle.step(30, &[0.0]);
            busy.step(30, &[1.0]);
        }
        assert!(flame_height(&busy) > flame_height(&idle) * 3);
    }

    #[test]
    fn edge_columns_follow_edge_cores() {
        let levels = [0.0, 0.5, 1.0];
        assert_eq!(column_level(&levels, 0, 30), 0.0);
        assert_eq!(column_level(&levels, 29, 30), 1.0);
        assert_eq!(column_level(&[0.4], 7, 30), 0.4);
    }

    #[test]
    fn tiny_terminals_dont_panic() {
        for (w, h) in [(0, 0), (1, 2), (3, 1), (1, 40)] {
            let mut fire = Fire::seeded(w, h, 3);
            fire.step(MAX_HEAT, &[1.0, 0.0]);
            fire.draw(&mut String::new());
        }
    }
}
