# lokprof

Benchmark a local inference server against another one, on the same prompts and the same
clock. It drives [Ollama](https://ollama.com), [LOKEN](https://github.com/loken-ai/loken)
and vLLM through their own APIs, sweeps models, context lengths and prompt sizes, and reports
what each of them actually did rather than what it claims.

```sh
lokprof --ollama http://localhost:11434 --loken http://localhost:11435 \
      --models qwen3:latest --num-ctx 4096 --prompts short,medium,long --stream
```

`--stream` is not optional if you intend to compare decode rates. Without it no engine can
report a client-observed first-token time, so the figure becomes tokens over the whole
wall clock - prefill included - for everyone, and the prefill/decode energy split does not
happen at all.

## What it measures, and why it is fussy about it

A benchmark that is easy to run is easy to run wrong, so this one is opinionated where the
difference between engines hides:

- **prefill and decode are separated.** A single tokens-per-second figure conflates the two,
  and they do not move together - one engine can win the decode and lose the request.
- **energy, not only speed.** GPU and host samplers run alongside the request, so a result
  carries joules per token beside tokens per second. Two engines at the same rate are not
  equivalent if one of them draws half again as much.

  Read the joules for what they are: **the machine's draw over the request window, not the
  engine's**. NVML sums every card present, RAPL returns whole CPU packages - operating
  system, this process and its own samplers included - and the idle floor is never subtracted,
  so part of every figure is `idle_power / throughput` and a slower engine is charged for
  occupying the machine longer. That is a defensible whole-system measurement **on an
  otherwise idle machine**, and meaningless on a box doing anything else. `--idle-energy-secs`
  records the idle floor in the JSON so you can subtract it yourself.

  Coverage differs by platform, and the reported labels say which domains are counted:
  CPU and DRAM energy come from Linux's powercap interface, so a run elsewhere - Windows
  included - counts **the GPU alone** and its J/token is mechanically lower. The same counters
  exist on other platforms but live in MSRs that only a kernel driver can read, and a
  benchmark has no business installing one.
- **the engines are made comparable before they are compared.** Which cards each engine may
  use is decided once, at launch, and applied to all of them, by the campaign in the
  [benchmarks](https://github.com/loken-ai/benchmarks) repository.
  Restricting one engine to a card while the others keep the machine is the easiest way to
  publish a fiction, and it is what a per-request pin quietly does. `--num-gpu 0` remains for
  the CPU-to-CPU cell, where the other engines are launched CPU-only alongside it.
- **vision suites** exist beside the text ones, because an image prompt exercises a different
  path and a text-only sweep says nothing about it.

## Reproducing a fair run

The tool measures; making the measurement mean something is the campaign's job, and it lives in
the [benchmarks](https://github.com/loken-ai/benchmarks) repository: one engine at a time with the others' processes stopped,
the same cards for every engine, cold restarts, warm-up discarded, greedy decoding and a thermal
gate. It drives this tool, and anyone can run it and share the runs there. Most of what the
campaign does exists because a run without it produced a figure that was wrong in a way nobody
noticed.

## Build

```sh
cargo build --release      # target/release/lokprof
```

No GPU or model is needed to build it. The GPU sampler reads NVML at run time when a card is
present and reports nothing when it is not.

## Licence

MIT OR Apache-2.0, at your option. See `LICENSE-MIT` and `LICENSE-APACHE`.
