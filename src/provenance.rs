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

impl Machine {
    /// The machine as the server that answered saw it: the cards it lists, not every card the
    /// vendor library finds. A campaign that hides the cards of one vendor from every engine
    /// then describes the cards it measured, whichever vendor they come from. A card the vendor
    /// library also knows keeps its details; any other is named as the server names it.
    pub fn seen_by(mut self, devices: &serde_json::Value) -> Self {
        let names: Vec<String> = devices
            .get("devices")
            .and_then(|d| d.as_array())
            .into_iter()
            .flatten()
            .filter(|d| d.get("status").and_then(|s| s.as_str()) == Some("available"))
            .filter(|d| {
                d.get("backend")
                    .or_else(|| d.get("type"))
                    .and_then(|t| t.as_str())
                    .is_some_and(|t| !t.eq_ignore_ascii_case("cpu"))
            })
            .filter_map(|d| d.get("name").and_then(|n| n.as_str()).map(str::to_string))
            .collect();
        if names.is_empty() {
            return self;
        }
        let mut known = std::mem::take(&mut self.gpus);
        let mut gpus = Vec::new();
        for (index, name) in names.iter().enumerate() {
            let mut gpu = match known.iter().position(|g| g.name.as_deref() == Some(name)) {
                Some(at) => known.remove(at),
                None => Gpu {
                    index: 0,
                    uuid: None,
                    name: Some(name.clone()),
                    vram_mib: None,
                    power_cap_w: None,
                },
            };
            gpu.index = index as u32;
            gpus.push(gpu);
        }
        if gpus.iter().all(|g| g.uuid.is_none()) {
            self.driver = None;
        }
        self.hardware_id = hardware_id(&self.cpu, self.threads, self.ram_gib, &gpus);
        self.gpus = gpus;
        self
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
/// The protocol says one engine at a time with the others stopped, and the campaign enforces it.
/// Nothing wrote down that they had, so a report taken under the rule and one taken beside a
/// running peer were the same file. This is what the runner can see for itself: the names, as
/// `/proc` gives them, of every process that could be serving a model.
///
/// Names rather than a verdict. "Alone" is a judgement about which of them was the target, and
/// the reader can make it from a list they can check.
pub fn engines_seen() -> Vec<String> {
    let mut seen: Vec<String> = engine_processes(std::path::Path::new("/proc"))
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    seen.dedup();
    seen
}

/// The engine processes under a `/proc` tree, as pairs of name and the file the process runs,
/// sorted by name then by path. A process whose `exe` cannot be read has no path.
fn engine_processes(proc_root: &std::path::Path) -> Vec<(String, Option<std::path::PathBuf>)> {
    const ENGINES: [&str; 4] = ["ollama", "lokend", "vllm", "llama-server"];
    let Ok(dir) = std::fs::read_dir(proc_root) else {
        return Vec::new();
    };
    let mut found: Vec<(String, Option<std::path::PathBuf>)> = Vec::new();
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
        if !ENGINES.contains(&comm.as_str()) {
            continue;
        }
        let exe = std::fs::read_link(path.join("exe")).ok();
        let item = (comm, exe);
        if !found.contains(&item) {
            found.push(item);
        }
    }
    found.sort();
    found
}

/// The binary of each engine running on this machine: its name and the SHA-256 of the file it
/// runs, or `null` when the file cannot be read.
/// Each distinct file is hashed once.
pub fn engine_binaries() -> serde_json::Value {
    engine_binaries_in(std::path::Path::new("/proc"))
}

fn engine_binaries_in(proc_root: &std::path::Path) -> serde_json::Value {
    let mut hashed: std::collections::HashMap<std::path::PathBuf, Option<String>> =
        std::collections::HashMap::new();
    let items: Vec<serde_json::Value> = engine_processes(proc_root)
        .into_iter()
        .map(|(name, exe)| {
            let sha = exe
                .map(|p| {
                    hashed
                        .entry(p.clone())
                        .or_insert_with(|| file_sha256(&p))
                        .clone()
                })
                .unwrap_or(None);
            serde_json::json!({ "name": name, "sha256": sha })
        })
        .collect();
    serde_json::Value::Array(items)
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
                "backend": d.get("backend"),
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

/// The command line, with every value that names a place on this machine replaced: a URL, a
/// path or a home directory says where the run happened, which the record never does.
pub fn redacted_command(args: impl IntoIterator<Item = String>) -> Vec<String> {
    let place = |v: &str| {
        v.contains("://")
            || v.starts_with('/')
            || v.starts_with('~')
            || v.starts_with('.')
            || v.contains('\\')
            || [
                ".json", ".csv", ".wav", ".txt", ".png", ".jpg", ".jpeg", ".gguf", ".toml",
            ]
            .iter()
            .any(|e| v.ends_with(e))
            || v.rsplit_once(':').is_some_and(|(h, p)| {
                !h.is_empty()
                    && p.chars().all(|c| c.is_ascii_digit())
                    && !p.is_empty()
                    && h.chars().any(|c| c == '.' || c.is_ascii_digit())
            })
    };
    args.into_iter()
        .enumerate()
        .map(|(i, a)| {
            if i == 0 {
                // The program, by its file name alone.
                return a.rsplit('/').next().unwrap_or(&a).to_string();
            }
            match a.split_once('=') {
                Some((flag, value)) if a.starts_with('-') && place(value) => {
                    format!("{flag}=<place>")
                }
                _ if !a.starts_with('-') && place(&a) => "<place>".to_string(),
                _ => a,
            }
        })
        .collect()
}

/// The hex SHA-256 of the file at `path`, `None` when it cannot be read.
pub fn file_sha256(path: &std::path::Path) -> Option<String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Some(format!("{:x}", hasher.finalize()))
}

/// What reproduces the run: the command, the binary that ran it, the binaries of the engines
/// running on this machine and a digest of the configuration the cells were measured under.
pub fn recipe(command: Vec<String>, config: &serde_json::Value) -> serde_json::Value {
    let binary = std::env::current_exe().ok().and_then(|p| file_sha256(&p));
    serde_json::json!({
        "command": command,
        "binary_sha256": binary,
        "engine_binaries": engine_binaries(),
        "config_sha256": format!("{:x}", Sha256::digest(config.to_string().as_bytes())),
    })
}

/// The placeholder the seal holds while the record is hashed.
pub const SEAL_BLANK: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// The record text with its seal filled in. The seal is the SHA-256 of the text written with
/// [`SEAL_BLANK`] in its place, so any reader hashes the file the same way after blanking the
/// seal. A copy that changes under the same name no longer matches. It detects change, it does
/// not authenticate who wrote the record.
pub fn sealed(mut payload: serde_json::Value) -> String {
    payload["seal"] = serde_json::Value::String(SEAL_BLANK.to_string());
    let blank = serde_json::to_string_pretty(&payload).unwrap_or_default();
    let digest = format!("{:x}", Sha256::digest(blank.as_bytes()));
    blank.replacen(SEAL_BLANK, &digest, 1)
}

/// Whether the record text holds the seal of its own content.
pub fn seal_holds(text: &str) -> bool {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else {
        return false;
    };
    let Some(seal) = v["seal"].as_str().map(str::to_string) else {
        return false;
    };
    let blank = text.replacen(&seal, SEAL_BLANK, 1);
    format!("{:x}", Sha256::digest(blank.as_bytes())) == seal
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_command_keeps_the_flags_and_drops_the_places() {
        let args = [
            "/home/someone/bin/lokprof",
            "--loken",
            "http://lokend.example:11435",
            "--models",
            "Qwen/Qwen3-8B-AWQ",
            "--audio=/data/sample.wav",
            "-o",
            "results-myhost.json",
            "--ollama",
            "engine.example:11434",
            "-n",
            "3",
        ]
        .map(String::from);
        assert_eq!(
            redacted_command(args),
            [
                "lokprof",
                "--loken",
                "<place>",
                "--models",
                "Qwen/Qwen3-8B-AWQ",
                "--audio=<place>",
                "-o",
                "<place>",
                "--ollama",
                "<place>",
                "-n",
                "3"
            ]
        );
    }

    #[test]
    fn a_sealed_record_stops_matching_when_it_changes() {
        let text = sealed(serde_json::json!({ "results": [1, 2, 3] }));
        assert!(seal_holds(&text));
        assert!(!seal_holds(&text.replace('3', "4")));
        assert!(!seal_holds("{}"));
    }

    use super::*;

    #[test]
    fn the_config_digest_ignores_key_order() {
        let a = serde_json::json!({ "models": ["m"], "iterations": 3 });
        let b = serde_json::json!({ "iterations": 3, "models": ["m"] });
        let c = serde_json::json!({ "iterations": 4, "models": ["m"] });
        let digest = |v: &serde_json::Value| recipe(Vec::new(), v)["config_sha256"].clone();
        assert_eq!(digest(&a), digest(&b));
        assert_ne!(digest(&a), digest(&c));
        let expected = format!("{:x}", Sha256::digest(a.to_string().as_bytes()));
        assert_eq!(digest(&a), serde_json::Value::String(expected));
    }

    #[test]
    fn a_file_digest_is_the_sha256_of_its_bytes() {
        let dir = scratch("file-digest");
        let file = dir.join("data.bin");
        std::fs::write(&file, b"abc").unwrap();
        assert_eq!(
            file_sha256(&file).as_deref(),
            Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
        assert_eq!(file_sha256(&dir.join("absent")), None);
    }

    #[test]
    fn the_recipe_hashes_the_running_binary() {
        let r = recipe(vec!["lokprof".into()], &serde_json::json!({}));
        let exe = std::env::current_exe().unwrap();
        assert_eq!(
            r["binary_sha256"].as_str().map(String::from),
            file_sha256(&exe)
        );
        assert_eq!(r["command"], serde_json::json!(["lokprof"]));
    }

    /// A fake `/proc` with 2 engines running the same file and 1 process that is no engine.
    #[test]
    fn engine_binaries_are_hashed_once_per_file() {
        let root = scratch("fake-proc");
        let exe = root.join("engine-file");
        std::fs::write(&exe, b"abc").unwrap();
        for (pid, comm) in [("11", "lokend"), ("12", "lokend"), ("13", "bash")] {
            let p = root.join(pid);
            std::fs::create_dir_all(&p).unwrap();
            std::fs::write(p.join("comm"), format!("{comm}\n")).unwrap();
            let _ = std::os::unix::fs::symlink(&exe, p.join("exe"));
        }
        let got = engine_binaries_in(&root);
        assert_eq!(
            got,
            serde_json::json!([{
                "name": "lokend",
                "sha256": "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
            }])
        );
    }

    #[test]
    fn a_sealed_record_does_not_depend_on_key_order() {
        let a = sealed(serde_json::json!({ "x": 1, "y": { "b": 2, "a": 3 } }));
        let b = sealed(serde_json::json!({ "y": { "a": 3, "b": 2 }, "x": 1 }));
        assert_eq!(a, b);
        assert!(seal_holds(&a));
    }

    #[test]
    fn a_one_byte_change_breaks_the_seal() {
        let text = sealed(serde_json::json!({ "results": [{ "tok_s": 100.5 }] }));
        assert!(seal_holds(&text));
        assert!(!seal_holds(&text.replace("100.5", "100.6")));
        let seal = serde_json::from_str::<serde_json::Value>(&text).unwrap()["seal"]
            .as_str()
            .unwrap()
            .to_string();
        let mut other = seal.clone();
        other.replace_range(0..1, if seal.starts_with('a') { "b" } else { "a" });
        assert!(!seal_holds(&text.replacen(&seal, &other, 1)));
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-scratch")
            .join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A machine described by the cards a server lists: the card of another vendor is named as
    /// the server names it, and the hardware id follows the cards that were measured.
    #[test]
    fn a_machine_is_described_by_the_cards_the_server_saw() {
        let seen = serde_json::json!({"devices": [
            {"type": "CUDA", "backend": "rocm", "name": "AMD Radeon RX 9070 XT", "status": "available"},
            {"type": "CPU", "backend": "cpu", "name": "CPU (20 cores)", "status": "available"}]});
        let all = machine();
        let amd = machine().seen_by(&seen);
        assert_eq!(amd.gpus.len(), 1);
        assert_eq!(amd.gpus[0].name.as_deref(), Some("AMD Radeon RX 9070 XT"));
        assert_ne!(amd.hardware_id, all.hardware_id);
        assert!(amd.gpus[0].uuid.is_none() && amd.driver.is_none());
        let silent = machine().seen_by(&serde_json::json!({"devices": []}));
        assert_eq!(silent.hardware_id, all.hardware_id);
    }

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
        // The same vector is pinned in the benchmarks repository: both sides must agree.
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
