//! What produced a measurement: the machine it ran on and the engine that answered.
//!
//! A rate without these is not comparable to another rate. Two runs from two driver versions,
//! two kernels or two builds of the same engine are indistinguishable in a report that records
//! none of them, and the difference between them is the size of the effects being measured.
//!
//! Everything here is read, never asked of a person. A field a human types is a field that is
//! right on the day somebody cares and wrong every day after.
//!
//! What is deliberately absent: the hostname, and any identifier that names a person. Two
//! identifiers exist and both are one-way hashes: `host_id` groups the runs of one machine
//! across card swaps, `hardware_id` names a configuration and is shared by every machine built
//! the same way. Neither can be turned back into a name or a serial number.

use serde::Serialize;
use sha2::{Digest, Sha256};

/// The shape of the record. Bumped when a rule changes what a number means, never for a new
/// field: a reader compares majors and refuses across them.
/// Records written before the schema was named loken carry the tool's old name; readers accept both.
pub const SCHEMA: &str = "loken/2";

/// The machine, as it describes itself.
#[derive(Debug, Serialize)]
pub struct Machine {
    /// One machine, whatever cards it holds: a hash of the OS machine id, salted so the id
    /// itself never leaves the host. Absent where the OS offers none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_id: Option<String>,
    /// One hardware configuration: cards (model, memory, power cap), processor, threads and
    /// memory. Card serials, driver and OS are left out, so two machines built alike share it
    /// and one machine that changes a card does not.
    pub hardware_id: String,
    /// The processor, as `/proc/cpuinfo` names it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu: Option<String>,
    /// Hardware threads, which is what a decode loop competes for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threads: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ram_gib: Option<u64>,
    /// The distribution, for the libraries around the engine.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os: Option<String>,
    /// The kernel, which decides what the driver can do.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kernel: Option<String>,
    /// The NVIDIA driver. A driver change moves decode rates, and nothing else in a report
    /// would say so.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub driver: Option<String>,
    pub gpus: Vec<Gpu>,
}

/// One card, identified rather than counted.
///
/// The UUID matters more than the name: two cards of the same model in one box are the same
/// string and different silicon, and which one a run landed on changes the result.
#[derive(Debug, Serialize)]
pub struct Gpu {
    pub index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vram_mib: Option<u64>,
    /// The cap in force, not the nameplate. A card held at 180 W and the same card at 300 W
    /// are two different measurements wearing one model number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub power_cap_w: Option<u32>,
}

/// Describe the machine. Every field is optional because every source of it can be absent -
/// a container without `/proc`, a host without a driver - and a missing fact must read as
/// missing rather than as a default.
pub fn machine() -> Machine {
    let cpu = cpu_model();
    let threads = cpu_threads();
    let ram_gib = ram_gib();
    let gpus = gpus();
    Machine {
        host_id: host_id(),
        hardware_id: hardware_id(&cpu, threads, ram_gib, &gpus),
        cpu,
        threads,
        ram_gib,
        os: os_pretty_name(),
        kernel: first_line("/proc/sys/kernel/osrelease"),
        driver: nvml_driver(),
        gpus,
    }
}

/// Sixteen hex digits of the SHA-256 of `parts`, each followed by a separator byte so that
/// ("ab", "c") and ("a", "bc") differ.
fn short_hash(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0u8]);
    }
    hasher.finalize()[..8]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn host_id() -> Option<String> {
    let id = first_line("/etc/machine-id")?;
    Some(short_hash(&["loken/host", &id]))
}

/// The cards enter sorted, so the order NVML indexes them in does not change the id.
fn hardware_id(
    cpu: &Option<String>,
    threads: Option<usize>,
    ram_gib: Option<u64>,
    gpus: &[Gpu],
) -> String {
    let mut cards: Vec<String> = gpus
        .iter()
        .map(|g| {
            format!(
                "{}/{}/{}",
                g.name.as_deref().unwrap_or("?"),
                g.vram_mib.map_or("?".into(), |v| v.to_string()),
                g.power_cap_w.map_or("?".into(), |w| w.to_string()),
            )
        })
        .collect();
    cards.sort();
    let cards = cards.join(",");
    let threads = threads.map_or("?".into(), |t| t.to_string());
    let ram = ram_gib.map_or("?".into(), |r| r.to_string());
    short_hash(&[
        "loken/hardware",
        &cards,
        cpu.as_deref().unwrap_or("?"),
        &threads,
        &ram,
    ])
}

fn first_line(path: &str) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()?
        .lines()
        .next()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
}

fn cpu_model() -> Option<String> {
    let text = std::fs::read_to_string("/proc/cpuinfo").ok()?;
    text.lines()
        .find(|l| l.starts_with("model name"))
        .and_then(|l| l.split_once(':'))
        .map(|(_, v)| v.trim().to_string())
}

fn cpu_threads() -> Option<usize> {
    let text = std::fs::read_to_string("/proc/cpuinfo").ok()?;
    let n = text.lines().filter(|l| l.starts_with("processor")).count();
    (n > 0).then_some(n)
}

fn ram_gib() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    let kb: u64 = text
        .lines()
        .find(|l| l.starts_with("MemTotal:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()?;
    Some(kb / 1024 / 1024)
}

fn os_pretty_name() -> Option<String> {
    let text = std::fs::read_to_string("/etc/os-release").ok()?;
    text.lines()
        .find(|l| l.starts_with("PRETTY_NAME="))?
        .split_once('=')
        .map(|(_, v)| v.trim_matches('"').to_string())
}

fn nvml_driver() -> Option<String> {
    nvml_wrapper::Nvml::init().ok()?.sys_driver_version().ok()
}

/// The cards, in the order NVML indexes them.
///
/// Read once at the start of a run rather than sampled: none of it moves, and a report that
/// omits it cannot say which card a number came from.
fn gpus() -> Vec<Gpu> {
    let Ok(nvml) = nvml_wrapper::Nvml::init() else {
        return Vec::new();
    };
    let Ok(count) = nvml.device_count() else {
        return Vec::new();
    };
    (0..count)
        .filter_map(|index| {
            let device = nvml.device_by_index(index).ok()?;
            Some(Gpu {
                index,
                uuid: device.uuid().ok(),
                name: device.name().ok(),
                vram_mib: device.memory_info().ok().map(|m| m.total / 1024 / 1024),
                // Reported in milliwatts, and rounded here because a cap is set in watts.
                power_cap_w: device.power_management_limit().ok().map(|mw| mw / 1000),
            })
        })
        .collect()
}

/// The engine-shaped processes running on this machine, by name.
///
/// The protocol says one engine at a time with the others stopped, and the scripts enforce it.
/// Nothing wrote down that they had, so a report taken under the rule and one taken beside a
/// running peer were the same file. This is what the runner can see for itself: the names, as
/// `/proc` gives them, of every process that could be serving a model.
///
/// Names rather than a verdict. "Alone" is a judgement about which of them was the target, and
/// the reader can make it from a list they can check.
pub fn engines_seen() -> Vec<String> {
    const ENGINES: [&str; 4] = ["ollama", "lokend", "vllm", "llama-server"];
    let Ok(dir) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    let mut seen: Vec<String> = Vec::new();
    for entry in dir.flatten() {
        let path = entry.path();
        let Some(pid) = path.file_name().and_then(|f| f.to_str()) else {
            continue;
        };
        if !pid.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let Ok(comm) = std::fs::read_to_string(path.join("comm")) else {
            continue;
        };
        let comm = comm.trim().to_string();
        if ENGINES.contains(&comm.as_str()) && !seen.contains(&comm) {
            seen.push(comm);
        }
    }
    seen.sort();
    seen
}

/// What the cards read before the first request, in whole degrees.
///
/// A card measured warm is a slower card, and the ordering of a sweep decides which engine gets
/// the cold one. The harness gates on this; the record keeps what the gate let through, so a
/// reader can see it rather than trust it.
pub fn gpu_temps_c() -> Vec<u32> {
    let Ok(nvml) = nvml_wrapper::Nvml::init() else {
        return Vec::new();
    };
    let Ok(count) = nvml.device_count() else {
        return Vec::new();
    };
    (0..count)
        .filter_map(|index| {
            nvml.device_by_index(index)
                .ok()?
                .temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu)
                .ok()
        })
        .collect()
}

/// What a server says it is, asked of the server rather than assumed from a flag.
///
/// Recorded verbatim: every engine answers a different shape, and the reader of a record two
/// years from now is better served by what the server actually said than by our translation
/// of it. `None` when nothing answers, which is itself worth recording.
pub async fn engine_identity(http: &reqwest::Client, base_url: &str) -> Option<serde_json::Value> {
    let base = base_url.trim_end_matches('/');
    // Ollama and loken answer the first; vLLM answers the second.
    for path in ["/api/version", "/version"] {
        let url = format!("{base}{path}");
        if let Ok(resp) = http.get(&url).send().await {
            if resp.status().is_success() {
                if let Ok(v) = resp.json::<serde_json::Value>().await {
                    return Some(v);
                }
            }
        }
    }
    None
}

/// The devices a server says it detected and the backends it was built with, reduced to the
/// facts a comparison depends on: the type, name and state of each device, and the compiled
/// features. Live telemetry (temperature, power, free memory) is left out, so asking twice
/// gives the same answer.
///
/// Only loken answers this route; for any other engine the result is `None`, which a reader
/// must take as "not stated", never as "no accelerator".
pub async fn engine_devices(http: &reqwest::Client, base_url: &str) -> Option<serde_json::Value> {
    let url = format!("{}/api/distributed/devices", base_url.trim_end_matches('/'));
    let resp = http.get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body = resp.json::<serde_json::Value>().await.ok()?;
    let devices: Vec<serde_json::Value> = body
        .get("devices")?
        .as_array()?
        .iter()
        .map(|d| {
            serde_json::json!({
                "type": d.get("type"),
                "name": d.get("name"),
                "status": d.get("status"),
            })
        })
        .collect();
    Some(serde_json::json!({
        "devices": devices,
        "compiled": body.pointer("/summary/compiled_features"),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every field is optional, and a machine that cannot answer one of them still produces a
    /// record. The alternative - a default - is a fact that reads as measured and is not.
    #[test]
    fn a_machine_describes_itself_or_says_nothing() {
        let m = machine();
        // On the machines this runs on, /proc answers. Where it does not, the field is absent
        // rather than zero, which is the property being asserted.
        if let Some(threads) = m.threads {
            assert!(threads > 0);
        }
        if let Some(ram) = m.ram_gib {
            assert!(ram > 0);
        }
        for gpu in &m.gpus {
            assert!(gpu.uuid.is_some() || gpu.name.is_some());
        }
    }

    /// The same card in a different slot, or the same machine described twice, is one hardware
    /// configuration; a different power cap is another.
    #[test]
    fn hardware_id_ignores_slot_order_and_follows_the_cap() {
        let card = |index, cap| Gpu {
            index,
            uuid: Some(format!("GPU-{index}")),
            name: Some("Card".into()),
            vram_mib: Some(16000),
            power_cap_w: Some(cap),
        };
        let cpu = Some("Cpu".to_string());
        let a = hardware_id(&cpu, Some(8), Some(32), &[card(0, 300), card(1, 180)]);
        let b = hardware_id(&cpu, Some(8), Some(32), &[card(1, 180), card(0, 300)]);
        let c = hardware_id(&cpu, Some(8), Some(32), &[card(0, 300), card(1, 200)]);
        assert_eq!(a, b);
        assert_ne!(a, c);
        // The same vector is pinned in scripts/hardware.py: both sides must agree.
        assert_eq!(a, "9e3586b770949eea");
    }

    /// The schema is a major and a minor, and comparison is on the major.
    #[test]
    fn the_schema_names_a_major() {
        let (name, version) = SCHEMA.split_once('/').expect("schema names a version");
        assert_eq!(name, "loken");
        assert!(version.parse::<u32>().is_ok(), "{version}");
    }
}
