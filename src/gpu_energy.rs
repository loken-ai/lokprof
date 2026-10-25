//! The energy of the graphics cards that NVML does not cover.
//!
//! NVIDIA cards are read through NVML (see `energy`). An AMD card publishes its power through
//! the amdgpu driver in hwmon, a discrete Intel card its energy the same way, and an
//! integrated GPU draws from the processor's package, whose RAPL domain already counts it.
//! This module finds the cards of a machine and the file each one can be read from, and
//! integrates a power reading over a window.
//!
//! A card with no readable source is reported as missing, so that a total which leaves it out
//! is never stored as if it were complete.

use std::path::{Path, PathBuf};
use std::time::Instant;

/// Smallest dedicated memory, in MiB, of a device counted as a card. The smallest card a
/// campaign measures holds 4 GiB; the memory an integrated GPU borrows from the host is less.
/// The benchmarks repository applies the same threshold when it decides which cards a run
/// exposes to an engine.
pub const CARD_MIN_MIB: u64 = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vendor {
    Nvidia,
    Amd,
    Intel,
}

/// Where the energy of one card is read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// A cumulative energy counter, in microjoules.
    Counter(PathBuf),
    /// An instantaneous power reading, in microwatts, integrated over the window.
    Power(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    pub name: String,
    pub vendor: Vendor,
    /// `None` for a card this module cannot read: an NVIDIA card is NVML's, any other has none.
    pub source: Option<Source>,
}

fn read_trim(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
}

fn is_display(class: &str) -> bool {
    class.starts_with("0x0300") || class.starts_with("0x0380") || class.starts_with("0x1200")
}

/// The bus number of the PCI device a drm card sits on (`0000:03:00.0` is bus 3).
fn pci_bus(card_dir: &Path) -> Option<u8> {
    let target = std::fs::canonicalize(card_dir.join("device")).ok()?;
    let name = target.file_name()?.to_string_lossy().to_string();
    u8::from_str_radix(name.split(':').nth(1)?, 16).ok()
}

/// The reading of a card's hwmon, a counter when the driver offers one.
fn hwmon_source(device: &Path) -> Option<Source> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(device.join("hwmon"))
        .ok()?
        .flatten()
        .map(|e| e.path())
        .collect();
    dirs.sort();
    for dir in &dirs {
        if dir.join("energy1_input").is_file() {
            return Some(Source::Counter(dir.join("energy1_input")));
        }
    }
    for dir in &dirs {
        for file in ["power1_average", "power1_input"] {
            if dir.join(file).is_file() {
                return Some(Source::Power(dir.join(file)));
            }
        }
    }
    None
}

/// The cards listed under a drm directory (`/sys/class/drm` on a machine).
pub fn discover(drm: &Path) -> Vec<Card> {
    let Ok(entries) = std::fs::read_dir(drm) else {
        return Vec::new();
    };
    let mut cards = Vec::new();
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let Some(rest) = name.strip_prefix("card") else {
            continue;
        };
        if rest.is_empty() || !rest.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let dir = e.path();
        let device = dir.join("device");
        let (Some(vendor), Some(class)) = (
            read_trim(&device.join("vendor")),
            read_trim(&device.join("class")),
        ) else {
            continue;
        };
        if !is_display(&class) {
            continue;
        }
        match vendor.as_str() {
            "0x10de" => cards.push(Card {
                name: name.clone(),
                vendor: Vendor::Nvidia,
                source: None,
            }),
            "0x1002" => {
                let mib = read_trim(&device.join("mem_info_vram_total"))
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0)
                    / 1_048_576;
                // An integrated GPU is inside the processor's package.
                if mib >= CARD_MIN_MIB {
                    cards.push(Card {
                        name: name.clone(),
                        vendor: Vendor::Amd,
                        source: hwmon_source(&device),
                    });
                }
            }
            // An integrated GPU sits on bus 0 and is inside the processor's package; a card on
            // another bus has its own supply and must offer its own reading.
            "0x8086" if pci_bus(&dir).is_some_and(|b| b != 0) => {
                let source = match hwmon_source(&device) {
                    Some(Source::Counter(p)) => Some(Source::Counter(p)),
                    _ => None,
                };
                cards.push(Card {
                    name: name.clone(),
                    vendor: Vendor::Intel,
                    source,
                });
            }
            _ => {}
        }
    }
    cards.sort_by(|a, b| a.name.cmp(&b.name));
    cards
}

/// The cards that cannot be measured, by name: an NVIDIA card when NVML is not there to read
/// it, and any other card without a source.
pub fn unmeasurable(cards: &[Card], nvml_reads: bool) -> Vec<String> {
    cards
        .iter()
        .filter(|c| match c.vendor {
            Vendor::Nvidia => !nvml_reads,
            _ => c.source.is_none(),
        })
        .map(|c| c.name.clone())
        .collect()
}

/// The running energy of one card read through hwmon.
pub struct Meter {
    source: Source,
    start_uj: Option<u128>,
    last_t: Instant,
    last_w: Option<f64>,
    joules: f64,
}

impl Meter {
    pub fn new(source: &Source, now: Instant) -> Self {
        let mut meter = Meter {
            source: source.clone(),
            start_uj: None,
            last_t: now,
            last_w: None,
            joules: 0.0,
        };
        meter.sample(now);
        meter
    }

    /// Joules since the meter was created, read at `now`.
    pub fn sample(&mut self, now: Instant) -> f64 {
        match &self.source {
            Source::Counter(path) => {
                if let Some(uj) = read_trim(path).and_then(|s| s.parse::<u128>().ok()) {
                    let start = *self.start_uj.get_or_insert(uj);
                    self.joules = uj.saturating_sub(start) as f64 / 1_000_000.0;
                }
            }
            Source::Power(path) => {
                if let Some(uw) = read_trim(path).and_then(|s| s.parse::<f64>().ok()) {
                    let watts = uw / 1_000_000.0;
                    let dt = now.saturating_duration_since(self.last_t).as_secs_f64();
                    // Trapezoid between the previous reading and this one.
                    let prev = self.last_w.unwrap_or(watts);
                    self.joules += (prev + watts) / 2.0 * dt;
                    self.last_w = Some(watts);
                }
                self.last_t = now;
            }
        }
        self.joules
    }
}

/// The meters of every card with a source.
pub fn meters(cards: &[Card], now: Instant) -> Vec<Meter> {
    cards
        .iter()
        .filter_map(|c| c.source.as_ref())
        .map(|s| Meter::new(s, now))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn card(
        root: &Path,
        name: &str,
        vendor: &str,
        class: &str,
        vram_bytes: Option<u64>,
        hwmon: &[(&str, &str)],
    ) {
        let device = root.join(name).join("device");
        std::fs::create_dir_all(&device).unwrap();
        std::fs::write(device.join("vendor"), format!("{vendor}\n")).unwrap();
        std::fs::write(device.join("class"), format!("{class}\n")).unwrap();
        if let Some(b) = vram_bytes {
            std::fs::write(device.join("mem_info_vram_total"), format!("{b}\n")).unwrap();
        }
        if !hwmon.is_empty() {
            let h = device.join("hwmon").join("hwmon3");
            std::fs::create_dir_all(&h).unwrap();
            for (f, v) in hwmon {
                std::fs::write(h.join(f), format!("{v}\n")).unwrap();
            }
        }
    }

    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("lokprof-gpu-energy-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    const GIB: u64 = 1 << 30;

    #[test]
    fn an_amd_card_is_read_through_its_power_and_a_counter_wins_when_there_is_one() {
        let root = scratch("amd");
        card(
            &root,
            "card0",
            "0x1002",
            "0x030000",
            Some(16 * GIB),
            &[("power1_average", "48000000")],
        );
        card(
            &root,
            "card1",
            "0x1002",
            "0x030000",
            Some(16 * GIB),
            &[("power1_average", "1"), ("energy1_input", "5")],
        );
        let cards = discover(&root);
        assert_eq!(cards.len(), 2);
        assert!(matches!(cards[0].source, Some(Source::Power(_))));
        assert!(matches!(cards[1].source, Some(Source::Counter(_))));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_integrated_gpu_is_not_a_card() {
        let root = scratch("igpu");
        card(
            &root,
            "card0",
            "0x1002",
            "0x030000",
            Some(2 * GIB),
            &[("power1_input", "9000000")],
        );
        card(&root, "card1", "0x10de", "0x030000", None, &[]);
        let cards = discover(&root);
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].vendor, Vendor::Nvidia);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_card_without_a_reading_is_unmeasurable_and_nvidia_depends_on_nvml() {
        let root = scratch("missing");
        card(&root, "card0", "0x1002", "0x030000", Some(16 * GIB), &[]);
        card(&root, "card1", "0x10de", "0x030000", None, &[]);
        let cards = discover(&root);
        assert_eq!(unmeasurable(&cards, true), vec!["card0".to_string()]);
        assert_eq!(
            unmeasurable(&cards, false),
            vec!["card0".to_string(), "card1".to_string()]
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_device_that_is_not_a_display_is_left_out() {
        let root = scratch("audio");
        card(
            &root,
            "card0",
            "0x1002",
            "0x040300",
            Some(16 * GIB),
            &[("power1_average", "1")],
        );
        assert!(discover(&root).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn power_is_integrated_with_the_trapezoid_rule() {
        let root = scratch("power");
        let file = root.join("power1_average");
        std::fs::write(&file, "10000000\n").unwrap();
        let t0 = Instant::now();
        let mut meter = Meter::new(&Source::Power(file.clone()), t0);
        std::fs::write(&file, "30000000\n").unwrap();
        let joules = meter.sample(t0 + Duration::from_secs(2));
        // (10 W + 30 W) / 2 over 2 s.
        assert!((joules - 40.0).abs() < 1e-9, "{joules}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_counter_is_read_from_where_it_stood_when_the_meter_started() {
        let root = scratch("counter");
        let file = root.join("energy1_input");
        std::fs::write(&file, "5000000\n").unwrap();
        let t0 = Instant::now();
        let mut meter = Meter::new(&Source::Counter(file.clone()), t0);
        std::fs::write(&file, "12500000\n").unwrap();
        assert!((meter.sample(t0 + Duration::from_secs(1)) - 7.5).abs() < 1e-9);
        let _ = std::fs::remove_dir_all(&root);
    }
}
