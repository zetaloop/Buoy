use std::fs;
use std::io;

pub struct Cpu {
    total: u64,
    idle: u64,
}

fn cpu_times() -> io::Result<(u64, u64)> {
    let stat = fs::read_to_string("/proc/stat")?;
    let v: Vec<u64> = stat
        .lines()
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .skip(1)
        .filter_map(|s| s.parse().ok())
        .collect();
    Ok((
        v.iter().sum(),
        v.get(3).copied().unwrap_or(0) + v.get(4).copied().unwrap_or(0),
    ))
}

impl Cpu {
    pub fn new() -> io::Result<Self> {
        let (total, idle) = cpu_times()?;
        Ok(Self { total, idle })
    }

    /// Busy percentage since the previous call.
    pub fn usage(&mut self) -> io::Result<u32> {
        let (total, idle) = cpu_times()?;
        let (dt, di) = (total - self.total, idle - self.idle);
        (self.total, self.idle) = (total, idle);
        Ok(if dt == 0 {
            0
        } else {
            (100.0 * (dt - di) as f64 / dt as f64).round() as u32
        })
    }
}

pub fn memory() -> io::Result<u32> {
    let info = fs::read_to_string("/proc/meminfo")?;
    let field = |k: &str| {
        info.lines().find_map(|l| {
            l.strip_prefix(k)?
                .strip_prefix(':')?
                .split_whitespace()
                .next()?
                .parse::<f64>()
                .ok()
        })
    };
    let (total, available) = field("MemTotal")
        .zip(field("MemAvailable"))
        .ok_or_else(|| io::Error::other("bad /proc/meminfo"))?;
    Ok((100.0 * (1.0 - available / total)).round() as u32)
}

pub fn temperature() -> Option<u32> {
    let milli: f64 = fs::read_to_string("/sys/class/thermal/thermal_zone0/temp")
        .ok()?
        .trim()
        .parse()
        .ok()?;
    Some((milli / 1000.0).round() as u32)
}

/// Fan duty reported by argononed through its shared memory segment.
pub fn fan() -> Option<u32> {
    fs::read("/dev/shm/argonone")
        .ok()?
        .first()
        .map(|&b| u32::from(b))
}
