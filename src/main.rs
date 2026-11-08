mod canvas;
mod display;
mod font;
mod net;
mod system;
mod traffic;

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use canvas::{Canvas, H, W, text_width};
use display::Display;
use traffic::{RATE, Traffic};

const FPS: u32 = 20;
const PHASE_SECONDS: u64 = 300;

static STOP: AtomicBool = AtomicBool::new(false);

extern "C" fn stop(_: libc::c_int) {
    STOP.store(true, Ordering::Relaxed);
}

struct Status {
    cpu: u32,
    mem: u32,
    tmp: Option<u32>,
    fan: Option<u32>,
}

/// Number with three significant digits (one decimal below M) and its unit.
fn split(mut v: f64) -> (String, char) {
    for unit in ['K', 'M', 'G'] {
        v /= 1000.0;
        if v < 999.5 {
            let n = if v < 9.995 && unit != 'K' {
                format!("{v:.2}")
            } else if v < 99.95 {
                format!("{v:.1}")
            } else {
                format!("{v:.0}")
            };
            return (n, unit);
        }
    }
    (format!("{:.0}", v / 1000.0), 'T')
}

fn reading(c: &mut Canvas, x: i32, v: f64, mark: font::Glyph) {
    let (n, unit) = split(v);
    c.glyph(x, 8 - mark.len() as i32, mark);
    let e = c.number(x + 5, 1, &n);
    c.glyph(e + 1, 3, font::unit(unit));
}

/// Horizontal positions the text rows cycle through.
fn shift(phase: u32) -> i32 {
    // a gap in the sequence keeps any 3 px wide glyph column from staying lit
    [0, 1, 3, 2, 1, 0][phase as usize % 6]
}

fn header(c: &mut Canvas, t: &Traffic, phase: u32) {
    let shift = shift(phase);
    // mark, gap, three digits, decimal point, gap, unit
    const SLOT: i32 = 3 + 2 + 3 * 5 + 4 + 1 + 4;
    let gap = (W as i32 - SLOT * 3) as f32 / 4.0;
    let (down, up) = t.shown();
    for (i, (v, mark)) in [
        (up, font::UP),
        (down, font::DOWN),
        (t.today as f64, font::SUM),
    ]
    .into_iter()
    .enumerate()
    {
        reading(
            c,
            (gap + i as f32 * (SLOT as f32 + gap)).round() as i32 + shift,
            v,
            mark,
        );
    }
}

fn chart(c: &mut Canvas, t: &Traffic, phase: u32) {
    // the whole plot rides between two rows so the divider is not pinned to one
    let dy = (phase as i32 / 2) % 2;
    let mid = 31 - dy;
    let half = mid - 10;
    let top = 8 - dy;
    for (i, (&u, &d)) in t.up.iter().zip(&t.down).enumerate() {
        let x = i as i32;
        let bar = |v: f64| ((v / t.top * f64::from(half)).round() as i32).min(half);
        c.rect(x, mid - bar(u), 1, bar(u), true);
        c.rect(x, mid + 1, 1, bar(d), true);
        if u > t.top {
            c.rect(x, top, 1, 2, false);
        }
        if d > t.top {
            c.rect(x, mid + half - 1, 1, 2, false);
        }
    }
    // the row swap above already rests each divider row half the time
    c.rect(0, mid, W as i32, 1, true);
    if t.changed < 2 * FPS {
        let (n, unit) = split(t.want);
        let label = format!("{}{unit}", n.split('.').next().unwrap_or_default());
        let x = W as i32 - text_width(&label) - 2;
        c.rect(x - 2, mid + half - 7, text_width(&label) + 4, 8, false);
        c.text(x, mid + half - 5, &label);
    }
}

fn footer(c: &mut Canvas, s: &Status, phase: u32) {
    let show = |v: Option<u32>| v.map_or("-".into(), |v| v.to_string());
    let items = [
        ("CPU", s.cpu.to_string()),
        ("MEM", s.mem.to_string()),
        ("TMP", show(s.tmp)),
        ("FAN", show(s.fan)),
    ];
    let widths: Vec<i32> = items
        .iter()
        .map(|(l, v)| text_width(l) + 4 + text_width(v))
        .collect();
    let gap = (W as i32 - widths.iter().sum::<i32>()) as f32 / (items.len() + 1) as f32;
    let mut x = gap + shift(phase) as f32;
    for ((label, value), w) in items.iter().zip(widths) {
        let e = c.text(x.round() as i32, H as i32 - 6, label);
        c.text(e + 4, H as i32 - 6, value);
        x += w as f32 + gap;
    }
}

fn main() -> io::Result<()> {
    // SAFETY: the handler only stores to an atomic.
    unsafe {
        libc::signal(libc::SIGTERM, stop as *const () as libc::sighandler_t);
        libc::signal(libc::SIGINT, stop as *const () as libc::sighandler_t);
    }
    let mut display = Display::open()?;
    let mut canvas = Canvas([false; W * H]);
    let mut cpu = system::Cpu::new()?;
    let mut counter = net::Counter::default();
    counter.sample();
    let mut traffic = Traffic::new();
    let mut status = Status {
        cpu: 0,
        mem: system::memory()?,
        tmp: system::temperature(),
        fan: system::fan(),
    };
    let start = Instant::now();
    let mut frame = 0u64;
    while !STOP.load(Ordering::Relaxed) {
        frame += 1;
        if frame.is_multiple_of(u64::from(FPS / RATE)) {
            let (rx, tx) = counter.sample();
            traffic.sample(frame as f64 / f64::from(FPS), rx, tx);
        }
        traffic.frame();
        // shift the layout once every few minutes so no pixel stays lit for long
        let phase = (frame / (u64::from(FPS) * PHASE_SECONDS)) as u32;
        if frame.is_multiple_of(u64::from(FPS)) {
            status = Status {
                cpu: cpu.usage()?,
                mem: system::memory()?,
                tmp: system::temperature(),
                fan: system::fan(),
            };
        }
        canvas.clear();
        header(&mut canvas, &traffic, phase);
        chart(&mut canvas, &traffic, phase);
        footer(&mut canvas, &status, phase);
        display.show(&canvas);
        let next = start + Duration::from_secs(frame) / FPS;
        thread::sleep(next.saturating_duration_since(Instant::now()));
    }
    Ok(())
}
