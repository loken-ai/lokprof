# Changelog

All notable changes to lokprof. The format follows [Keep a Changelog](https://keepachangelog.com/1.1.0/),
and versions follow [semantic versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- Coherence: judge a streamed answer on its whole text, as a non-streamed one.

### Changed

- Project: rename assay to lokprof.
- Records: name the schema loken and still read the old name.
- Records: derive the host and hardware ids without the tool's name.
- Records: describe the machine by the cards the measured server saw.
- Records: name the backend of each device the server lists.

## [0.2.0] - 2026-10-18

### Added

- Metrics: server decode rate (eval_duration/eval_count), beside client rate.
- Engines: --loken-label reruns one, LOKEN_CWD alt config, LOKEN_ONLY isolates.
- Provenance: schema, machine, cards, server reports, rules asserted or null.
- Machine: host_id and hardware_id, so a card swap stays one host, a new hardware.
- Subjects: the devices and backends a server detected, recorded with its version.
- Comparison: a class names the hardware by its id, shared by builds alike.
- Store: runs filed once under their content hash, with a rebuilt index.
- Archive: an operator can attest the machine that took an old report.
- Archive: records taken from a named archive carry its label.
- Records: replayable measurement records brought in with their inputs kept.
- Conditions: engine processes and card readings before first request recorded.
- Comparison: class from schema major, protocol, flags, machine, builds; else unknown.
- Quarantine: submissions validated into a pool, promoted on a class match.
- Archive: converter carries an old report forward; provenance null, stamp kept.
- CI: fmt, clippy -Dwarnings, tests, docs, shellcheck, no path-deps, cargo deny.
- Report: unloadable cells for Ollama, vLLM, pre-flight, each with its reason.
- Modalities: image, tts, asr with word error rate, music; chosen by --modality.
- Engines: ENGINES names the engines a fair run measures.

### Changed

- Engines: started from binary dir; no config.toml is refused, not defaulted.
- Cleanup: stops the engine by its binary name, so no cell leaves it running.
- Comments: ASCII throughout: dashes, quotes, arrows, units, comparison signs.
- Coherence: no rate for a hex answer unless it spells printable text.
- Report: iteration errors kept on the cell, not dropped.
- Build: bytecode left by scripts is ignored.

### Fixed

- Merge: keeps provenance it dropped: schema, machine, rules, idle baseline.
- Merge: carries the older engine stamp, so the mixed-builds warning survives.
- Coherence: judges the whole answer at capture sites, not a 120-char preview.
- Timeouts: per-request removed so a long prefill survives; client-wide applies.
- Report: an empty answer is named as such, not blamed on the server.

### Security

- Security: deny.toml allows tree licences, bars yanked and unknown sources; h2 patched.
- Security: rustls 0.23.46, past four advisories on its TLS client.

## [0.1.0] - 2026-08-18

### Added

- Sweeps: models by context by prompt against Ollama, LOKEN, vLLM via each API.
- Metrics: prefill and decode apart; one tokens/sec conflates two rates.
- Energy: GPU and host samplers run with the request, giving joules per token.
- Metrics: one definition, all engines; rates, TTFT, latency from client clock.
- Energy: by domain, CPU and DRAM from Linux powercap; GPU-only run never mixes.
- Policy: gpu-policy.sh and fair-run.sh set each engine's cards at launch.
- Protocol: one engine at a time, cold restarts, thermal gate, ollama pin.
- Coherence: gate on cells lacking an expected answer; no rate for a degenerate.
- Vision: image prompt suites beside text ones; an image takes a different path.
- Concurrency: --concurrency measures what the engine serves, not one request.
- Config: --num-gpu 0 for a CPU-to-CPU run.
- Config: --session-id for prefix reuse where supported.
- Config: --unique-prompt to defeat prefix caching.
- Config: --host-proc-names for the memory-footprint columns.
- Energy: --idle-energy-secs records the idle floor.
