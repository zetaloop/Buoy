use std::fs;

/// Interface carrying the IPv4 default route.
fn default_interface() -> Option<String> {
    let routes = fs::read_to_string("/proc/net/route").ok()?;
    routes.lines().skip(1).find_map(|l| {
        let mut f = l.split_whitespace();
        let name = f.next()?;
        (f.next()? == "00000000").then(|| name.to_owned())
    })
}

/// Received and transmitted byte counters of `name`.
fn counters(name: &str) -> Option<(u64, u64)> {
    let dev = fs::read_to_string("/proc/net/dev").ok()?;
    dev.lines().find_map(|l| {
        let (n, rest) = l.split_once(':')?;
        if n.trim() != name {
            return None;
        }
        let f: Vec<u64> = rest
            .split_whitespace()
            .filter_map(|s| s.parse().ok())
            .collect();
        Some((*f.first()?, *f.get(8)?))
    })
}

/// Byte deltas on the default-route interface between calls.
#[derive(Default)]
pub struct Counter {
    last: Option<(String, u64, u64)>,
}

impl Counter {
    pub fn sample(&mut self) -> (u64, u64) {
        let now = default_interface().and_then(|n| counters(&n).map(|(rx, tx)| (n, rx, tx)));
        let delta = match (&self.last, &now) {
            (Some((a, rx0, tx0)), Some((b, rx1, tx1))) if a == b => {
                (rx1.saturating_sub(*rx0), tx1.saturating_sub(*tx0))
            }
            _ => (0, 0),
        };
        self.last = now;
        delta
    }
}
