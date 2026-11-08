# lokprof

A command line profiler that measures inference servers on the same prompts and the same clock.
It drives Ollama, [loken](https://github.com/loken-ai/loken) and vLLM through their own APIs, sweeps models, context lengths and prompts, and records what each server did.  
Prefill and decode are timed apart, and the energy of the machine is measured over every request.

```sh
cargo build --release
target/release/lokprof --ollama 11434 --loken 11435 \
    --models qwen3:0.6b --prompts short,medium,long --stream -o run.json
```

`--stream` gives each engine a client-observed time to first token and a decode rate.  
Without it, every rate is tokens over the whole request, prefill included.

## State

| What | State |
|---|---|
| Install | build from source with `cargo`, no prebuilt archive |
| Text sweeps | Ollama, loken and vLLM |
| Vision sweeps | Ollama and loken |
| Image, speech, music and transcription | through the OpenAI routes of the server |
| Energy of the cards | NVIDIA through NVML, AMD and discrete Intel cards through hwmon |
| Energy of the processor | Linux only, from RAPL |

## Documentation

| Page | Read it to |
|---|---|
| [Overview](https://loken-ai.github.io/lokprof/overview/) | See what lokprof measures and what it records |
| [Install](https://loken-ai.github.io/lokprof/install/) | Build lokprof |
| [First run](https://loken-ai.github.io/lokprof/first-run/) | Measure 1 server and save a record |
| [Concepts](https://loken-ai.github.io/lokprof/concepts/) | Learn the terms: target, cell, sweep, window |
| [Guides](https://loken-ai.github.io/lokprof/compare-engines/) | Do 1 task: compare servers, measure energy, vision, media, concurrency, load, replay, a baseline |
| [Options](https://loken-ai.github.io/lokprof/options/) | Look up a command line option and its default |
| [Record](https://loken-ai.github.io/lokprof/record/) | Read the JSON record and the CSV file |

A fair comparison between engines also needs a protocol: one engine at a time, the same cards, a thermal gate.
The [benchmarks](https://github.com/loken-ai/benchmarks) repository runs that campaign with lokprof and publishes the results.

## Contributing

[CONTRIBUTING.md](CONTRIBUTING.md) covers the build, the checks and the layout of the repository.

## Licence

MIT OR Apache-2.0, at your option.
