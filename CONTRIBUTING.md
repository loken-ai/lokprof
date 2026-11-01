# Contributing

Issues and pull requests are welcome.

## Build

```sh
cargo build --release      # target/release/lokprof
```

The build needs no card and no model.
NVML is loaded at run time.

## Checks

CI runs these on every push to `main` and every pull request, with warnings as errors:

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
cargo deny check
```

CI also refuses a `path` dependency in `Cargo.toml`, so the crate builds on its own.  
The tests need no server, no card and no model.
A test that needs a server starts a local one on a free port.

## Layout

| Path | Holds |
|---|---|
| `src/main.rs` | the options, the sweep, the console tables, the JSON record and the CSV file |
| `src/api.rs` | the client of each target: the requests, the stream reader, the media routes |
| `src/stats.rs` | the statistics and the results and comparison tables |
| `src/coherence.rs` | the test of an answer that loops or is made of raw bytes |
| `src/energy.rs` | the energy window: RAPL, the NVML counter, the decode split |
| `src/gpu_energy.rs` | the cards that NVML does not read: AMD and Intel through hwmon |
| `src/gpu_sampler.rs` | the samples of NVIDIA card memory, load and power |
| `src/host_sampler.rs` | the memory and processor use of the engine processes, from `/proc` |
| `src/provenance.rs` | the schema, the machine, the server identities, the engines running |
| `prompts/` | the long built-in prompts, compiled into the binary |
| `deny.toml` | the licences and sources `cargo deny` accepts |

## Documentation

The user documentation lives in `docs/`, as Markdown pages that the family site builds.
A page in `docs/start`, `docs/guides` or `docs/reference` names no source file.

The script that makes the processor's energy readable is `scripts/enable-energy.sh` in the [benchmarks](https://github.com/loken-ai/benchmarks) repository.
The user pages name it as the command `enable-energy.sh`.

## Changes that move a number

A change to how a figure is measured changes what the record means.
It bumps the major of `SCHEMA`, and says so in `CHANGELOG.md`.  
A new field leaves the schema as it is.
