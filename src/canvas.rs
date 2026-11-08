use crate::font::{self, Glyph};

pub const W: usize = 128;
pub const H: usize = 64;

pub struct Canvas(pub [bool; W * H]);

impl Canvas {
    pub fn clear(&mut self) {
        self.0 = [false; W * H];
    }

    pub fn set(&mut self, x: i32, y: i32, on: bool) {
        if (0..W as i32).contains(&x) && (0..H as i32).contains(&y) {
            self.0[y as usize * W + x as usize] = on;
        }
    }

    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, on: bool) {
        for j in 0..h {
            for i in 0..w {
                self.set(x + i, y + j, on);
            }
        }
    }

    pub fn glyph(&mut self, x: i32, y: i32, g: Glyph) -> i32 {
        for (j, row) in g.iter().enumerate() {
            for (i, b) in row.bytes().enumerate() {
                if b == b'1' {
                    self.set(x + i as i32, y + j as i32, true);
                }
            }
        }
        x + g[0].len() as i32
    }

    pub fn text(&mut self, mut x: i32, y: i32, s: &str) -> i32 {
        for c in s.chars() {
            x = self.glyph(x, y, font::small(c)) + 1;
        }
        x - 1
    }
}

pub fn text_width(s: &str) -> i32 {
    s.chars().count() as i32 * 4 - 1
}
