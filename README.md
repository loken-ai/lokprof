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

## Quality

A record says how far each answer departs from the answers of a reference.

| Option | What it does | Standard |
|---|---|---|
| `--top-logprobs N` | Asks each engine for the log-probability of every token and of the N most likely ones. The record holds each answer and these figures, token by token. | The `top_logprobs` field of the [OpenAI chat completions](https://platform.openai.com/docs/api-reference/chat/create), from 0 to 20 |
| `--quality-base FILE` | Compares the answers with those of a record written with `--top-logprobs`, in place of the first target of the run. The value names the record by `reference_record_sha256`. | The `--kl-divergence-base FILE` of [llama.cpp's perplexity tool](https://github.com/ggml-org/llama.cpp/tree/master/tools/perplexity): the file of a reference run that a later run compares with |

The reference of a cell is the first target of the reference with the same model, context and prompt.

| Engine | Route | Fields sent |
|---|---|---|
| Ollama, loken | `/api/generate` | `logprobs: true`, `top_logprobs: N` |
| vLLM | `/v1/completions` | `logprobs: N` |

With `--top-logprobs`, the `quality` value of a cell holds `logprobs`: the statistics of the KL divergence run of [llama.cpp's perplexity tool](https://github.com/ggml-org/llama.cpp/tree/master/tools/perplexity), over what 2 free answers share.

| Field | What it holds |
|---|---|
| `shared_tokens` | The tokens both answers share from the start |
| `delta_p` | `p - p_ref` of each shared token: mean, RMS, median, minimum, maximum |
| `mean_ln_ppl_ratio` | The mean of `ln p_ref - ln p` over the shared tokens |
| `same_top_p` | The share of the positions with the same preceding tokens whose most likely token is the same |

The engines send the top tokens only, so no KL divergence over the vocabulary is given.
`logprobs` is `null` when an engine sends no log-probabilities, and `logprobs_reason` says why.

## Replay

`--replay FILE` sends the requests of a trace at the times the trace holds.
The record names the file by the SHA-256 of its bytes.

| Format | Told apart by | Prompt sent | Standard |
|---|---|---|---|
| Text | `offset_ms` | The text of the line, after the earlier lines of its session | lokprof's own, as in `tests/trace.jsonl` |
| Mooncake | `timestamp` | 1 word per token. Each block of 512 tokens is drawn from its hash, so requests that share hashes share a prefix. | The [Mooncake trace](https://github.com/kvcache-ai/Mooncake/tree/main/FAST25-release) that the `mooncake_trace` dataset type of [AIPerf](https://github.com/ai-dynamo/aiperf) replays |

A Mooncake request asks for `output_length` tokens, and its offset is its `timestamp` minus that of the first line.
`tests/mooncake_toolagent.jsonl` holds 2 recorded agent sessions of the Mooncake tool and agent trace, see [NOTICE](NOTICE).

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
