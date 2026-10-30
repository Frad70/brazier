//! CPU load from /proc/stat and temperature from hwmon.

use std::fs;
use std::io;
use std::path::PathBuf;

#[derive(Clone, Copy, Default, Debug, PartialEq)]
struct Ticks {
    busy: u64,
    total: u64,
}

impl Ticks {
    // Only the first eight counters are summed, guest time is already
    // counted in user and nice.
    fn parse(line: &str) -> Option<Ticks> {
        let mut total = 0;
        let mut idle = 0;
        for (i, field) in line.split_whitespace().skip(1).take(8).enumerate() {
            let n: u64 = field.parse().ok()?;
            total += n;
            if i == 3 || i == 4 {
                idle += n; // idle and iowait
            }
        }
        Some(Ticks { busy: total - idle, total })
    }

    fn usage_since(self, prev: Ticks) -> f32 {
        let total = self.total.saturating_sub(prev.total);
        if total == 0 {
            return 0.0;
        }
        (self.busy.saturating_sub(prev.busy) as f32 / total as f32).clamp(0.0, 1.0)
    }
}

fn parse_stat(text: &str) -> (Ticks, Vec<Ticks>) {
    let mut all = Ticks::default();
    let mut cores = Vec::new();
    for line in text.lines() {
        if line.starts_with("cpu ") {
            all = Ticks::parse(line).unwrap_or_default();
        } else if line.starts_with("cpu") {
            cores.extend(Ticks::parse(line));
        }
    }
    (all, cores)
}

fn read_stat() -> io::Result<(Ticks, Vec<Ticks>)> {
    let text = fs::read_to_string("/proc/stat")
        .map_err(|e| io::Error::new(e.kind(), format!("/proc/stat: {e}")))?;
    Ok(parse_stat(&text))
}

pub struct Load {
    pub total: f32,
    pub cores: Vec<f32>,
}

pub struct Cpu {
    all: Ticks,
    cores: Vec<Ticks>,
}

impl Cpu {
    pub fn open() -> io::Result<Cpu> {
        let (all, cores) = read_stat()?;
        if cores.is_empty() {
            return Err(io::Error::other("no per-core lines in /proc/stat"));
        }
        Ok(Cpu { all, cores })
    }

    /// Load since the previous call, from 0.0 to 1.0.
    pub fn sample(&mut self) -> io::Result<Load> {
        let (all, cores) = read_stat()?;
        let total = all.usage_since(self.all);
        // a core going offline shifts the list, so just use the total for one sample
        let per_core = if cores.len() == self.cores.len() {
            cores.iter().zip(&self.cores).map(|(now, prev)| now.usage_since(*prev)).collect()
        } else {
            vec![total; cores.len()]
        };
        self.all = all;
        self.cores = cores;
        Ok(Load { total, cores: per_core })
    }
}

// Preferred sensors, best first. temp1 is the package/Tctl reading on all of them.
const SENSORS: [&str; 5] = ["k10temp", "zenpower", "coretemp", "cpu_thermal", "acpitz"];

pub struct Thermometer {
    input: Option<PathBuf>,
}

impl Thermometer {
    pub fn find() -> Thermometer {
        let mut found = Vec::new();
        if let Ok(dir) = fs::read_dir("/sys/class/hwmon") {
            for entry in dir.flatten() {
                let name = fs::read_to_string(entry.path().join("name")).unwrap_or_default();
                let input = entry.path().join("temp1_input");
                if let Some(rank) = SENSORS.iter().position(|s| *s == name.trim()) {
                    if input.exists() {
                        found.push((rank, input));
                    }
                }
            }
        }
        found.sort();
        Thermometer { input: found.into_iter().next().map(|(_, path)| path) }
    }

    /// Degrees Celsius, or None if there's no sensor we know.
    pub fn read(&self) -> Option<f32> {
        let raw = fs::read_to_string(self.input.as_ref()?).ok()?;
        raw.trim().parse::<f32>().ok().map(|millis| millis / 1000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STAT: &str = "\
cpu  100 0 50 800 50 0 0 0 0 0
cpu0 60 0 20 400 20 0 0 0 0 0
cpu1 40 0 30 400 30 0 0 0 0 0
intr 12345 0 0
ctxt 999
";

    #[test]
    fn parses_total_and_cores() {
        let (all, cores) = parse_stat(STAT);
        assert_eq!(all, Ticks { busy: 150, total: 1000 });
        assert_eq!(cores, vec![Ticks { busy: 80, total: 500 }, Ticks { busy: 70, total: 500 }]);
    }

    #[test]
    fn usage_is_share_of_busy_ticks() {
        let prev = Ticks { busy: 100, total: 1000 };
        let now = Ticks { busy: 175, total: 1100 };
        assert_eq!(now.usage_since(prev), 0.75);
    }

    #[test]
    fn no_new_ticks_means_no_load() {
        let t = Ticks { busy: 5, total: 10 };
        assert_eq!(t.usage_since(t), 0.0);
    }
}
