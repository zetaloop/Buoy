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
    high: u32,
    /// Samples since the scale last grew; holds off shrinking right after.
    age: u32,
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

/// The rung of the 1/2/5 ladder just below `v`.
fn lower(v: f64) -> f64 {
    let mut step = 1000.0;
    loop {
        for (i, m) in [1.0, 2.0, 5.0].into_iter().enumerate() {
            if m * step >= v {
                return if i == 0 {
                    step / 2.0
                } else {
                    [1.0, 2.0, 5.0][i - 1] * step
                };
            }
        }
        step *= 10.0;
    }
}

/// Chart-wide 90th percentile, so a narrow set of tall peaks does not hold the scale up.
fn p90(down: &[f64], up: &[f64]) -> f64 {
    let mut v: Vec<f64> = down.iter().chain(up).copied().collect();
    v.sort_by(f64::total_cmp);
    v[v.len() * 9 / 10]
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
            high: 0,
            age: 0,
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
        self.age += 1;
        // only traffic that lasts 2.5 s sets the scale, so a page burst stays
        // clipped instead of rescaling the chart and back
        let n = (RATE * 5 / 2) as usize;
        let steady = (W - n..W).fold(f64::INFINITY, |a, i| a.min(self.down[i].max(self.up[i])));
        let need = nice((steady * 1.6).max(200e3));
        // the history keeps the scale up while a large part of the visible chart
        // still has something to look at; the newest samples alone decide growth
        let hist = nice((p90(&self.down, &self.up) * 1.6).max(200e3));
        // a dead band either side of the current scale keeps traffic that hovers
        // near a rung from flapping between two of them
        if need > self.want * 1.25 {
            // a sustained transfer still opens up within a second, a burst of
            // requests that briefly pushes the same band does not
            self.high += 1;
            if self.high > RATE / 2 {
                (self.want, self.low, self.high, self.changed) = (need, 0, 0, 0);
                self.age = 0;
            }
        } else if need < self.want / 1.5 && hist < self.want / 1.5 && self.age > RATE * 4 {
            // a scale that just grew should stay put long enough to be read
            self.low += 1;
            if self.low > RATE / 2 {
                // a scale that just grew is still being read: step it down slowly
                let from = if hist < self.want / 4.0 && self.age > RATE * 8 {
                    need
                } else {
                    lower(self.want)
                };
                (self.want, self.low, self.changed) = (from.max(need), 0, 0);
            }
        } else {
            (self.low, self.high) = (0, 0);
        }
    }

    /// Advances the scale animation by one frame.
    pub fn frame(&mut self) {
        self.top *= (self.want / self.top).powf(0.35);
        self.changed += 1;
    }
}
