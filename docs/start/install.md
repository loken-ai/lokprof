# Install

lokprof builds from source into 1 binary, `lokprof`.
The project publishes no prebuilt archive and no package.

## Before you start

<!-- site: widget checklist id=lokprof-install-prereq -->
- [ ] A Rust toolchain with `cargo`.
- [ ] A checkout of `https://github.com/loken-ai/lokprof`.
- [ ] For the energy of the processor and the footprint of the engine processes: Linux.
- [ ] For the energy and the samples of NVIDIA cards: the NVIDIA driver, which provides NVML.
<!-- site: end -->

The build needs no card and no model.
lokprof loads NVML when it runs, and a machine without it reports no NVIDIA figure.

## Build

<!-- site: widget tabs group=install id=lokprof-install-method -->
### Build in the checkout

```sh
cargo build --release
```

The binary is `target/release/lokprof`.

### Install with cargo

```sh
cargo install --path .
```

`cargo` puts `lokprof` in its binary directory, `~/.cargo/bin` by default.
<!-- site: end -->

## Verify

<!-- site: widget checklist id=lokprof-install-verify -->
- [ ] `lokprof --version` prints `lokprof` and its version.
- [ ] `lokprof --help` lists the options.
- [ ] `lokprof` with no option prints `Error: specify at least one of --ollama PORT, --loken PORT, or --vllm PORT` and exits with status 1.
<!-- site: end -->
