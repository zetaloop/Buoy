use crate::canvas::W;

/// Samples per second; the chart scrolls one pixel per sample.
pub const RATE: u32 = 4;

/// Shown rate of one direction, refreshed faster as it gets higher.
struct Reader {
    next: f64,
    interval: f64,
    value: f64,
}

impl Reader {
    fn update(&mut self, t: f64, v: f64) {
        if t + 1e-9 < self.next {
            return;
        }
        // 2 s at 10K, 0.5 s at 1M, 0.25 s from 10M
        let level = v.clamp(10e3, 10e6);
        let target = 2.0 * 0.125f64.powf((level / 10e3).log10() / 3.0);
        let mut step = if target <= self.interval {
            target
        } else {
            (self.interval + 0.35).min(target)
        };
        // drift toward the grid counted from start so slow readings settle in step
        let next = t + step;
        let grid = (next / target).round() * target;
        step += (grid - next).clamp(-0.2 * step, 0.2 * step);
        self.interval = step.min(target);
        self.next = t + step;
        self.value = v;
    }
}

pub struct Traffic {
    pub down: Vec<f64>,
    pub up: Vec<f64>,
    pub today: u64,
    day: i32,
    readers: [Reader; 2],
    /// Target chart scale and how long a smaller one has been wanted.
    pub want: f64,
    low: u32,
    /// Drawn scale, easing toward `want`.
    pub top: f64,
    /// Frames since `want` last changed.
    pub changed: u32,
}

fn nice(v: f64) -> f64 {
    let mut step = 1000.0;
    loop {
        for m in [1.0, 2.0, 5.0] {
            if m * step >= v {
                return m * step;
            }
        }
        step *= 10.0;
    }
}

fn yday() -> i32 {
    // SAFETY: localtime_r only writes into `tm`.
    unsafe {
        let now = libc::time(std::ptr::null_mut());
        let mut tm = std::mem::zeroed();
        libc::localtime_r(&now, &mut tm);
        tm.tm_yday
    }
}

impl Traffic {
    pub fn new() -> Self {
        let reader = || Reader {
            next: 0.0,
            interval: 1.0,
            value: 0.0,
        };
        Self {
            down: vec![0.0; W],
            up: vec![0.0; W],
            today: 0,
            day: yday(),
            readers: [reader(), reader()],
            want: 1000.0,
            low: 0,
            top: 1000.0,
            changed: 0,
        }
    }

    pub fn shown(&self) -> (f64, f64) {
        (self.readers[0].value, self.readers[1].value)
    }

    /// Records one sample of byte deltas taken at `t` seconds since start.
    pub fn sample(&mut self, t: f64, rx: u64, tx: u64) {
        let day = yday();
        if day != self.day {
            (self.day, self.today) = (day, 0);
        }
        self.today += rx + tx;
        let rate = f64::from(RATE);
        for (h, v) in [(&mut self.down, rx), (&mut self.up, tx)] {
            h.remove(0);
            h.push(v as f64 * rate);
        }
        let n = RATE as usize;
        let avg = |h: &[f64]| h[W - n..].iter().sum::<f64>() / n as f64;
        let (d, u) = (avg(&self.down), avg(&self.up));
        self.readers[0].update(t, d);
        self.readers[1].update(t, u);
        self.retarget();
    }

    fn retarget(&mut self) {
        // weighted p90: the oldest quarter fades from weight 1 to 0.3
        let q = (W / 4) as f64;
        let mut pairs: Vec<(f64, f64)> = (0..W)
            .flat_map(|i| {
                [self.down[i], self.up[i]].map(|v| (v, 0.3 + 0.7 * (i as f64 / q).min(1.0)))
            })
            .collect();
        pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
        let total: f64 = pairs.iter().map(|p| p.1).sum();
        let mut acc = 0.0;
        let mut v = 0.0;
        for (x, w) in pairs {
            acc += w;
            v = x;
            if acc >= total * 0.9 {
                break;
            }
        }
        v *= 1.3;
        // a sustained climb (1.5 s all above scale, still rising) expands at once
        let recent: Vec<f64> = (W - 6..W).map(|i| self.down[i].max(self.up[i])).collect();
        if recent.iter().all(|&r| r > self.want) && recent[5] >= recent[0] {
            v = v.max(recent[5] * 1.3);
        }
        // soft floor keeps background traffic low in the chart
        let want = nice((v * v + 200e3 * 200e3).sqrt());
        if want > self.want {
            (self.want, self.low, self.changed) = (want, 0, 0);
        } else if want < self.want {
            self.low += 1;
            if self.low > RATE * 3 {
                (self.want, self.low, self.changed) = (want, 0, 0);
            }
        } else {
            self.low = 0;
        }
    }

    /// Advances the scale animation by one frame.
    pub fn frame(&mut self) {
        self.top *= (self.want / self.top).powf(0.35);
        self.changed += 1;
    }
}
