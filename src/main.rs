mod canvas;
mod display;
mod font;
mod system;

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use canvas::{Canvas, H, W, text_width};
use display::Display;

const FPS: u32 = 20;

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

fn footer(c: &mut Canvas, s: &Status) {
    c.rect(0, 56, W as i32, 1, true);
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
    let mut x = gap;
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
        if frame.is_multiple_of(u64::from(FPS)) {
            status = Status {
                cpu: cpu.usage()?,
                mem: system::memory()?,
                tmp: system::temperature(),
                fan: system::fan(),
            };
        }
        canvas.clear();
        footer(&mut canvas, &status);
        display.show(&canvas);
        let next = start + Duration::from_secs(frame) / FPS;
        thread::sleep(next.saturating_duration_since(Instant::now()));
    }
    Ok(())
}
