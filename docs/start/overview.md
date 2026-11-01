# lokprof

A command line profiler that sends the same prompts to several inference servers and measures each of them on the client's clock.

> [!NOTE]
> Every server is measured by 1 definition.
> The time to first token, the decode rate and the latency come from the clock of lokprof, never from what a server reports of itself.  
> The energy is that of the whole machine over the request, and a figure is stored only when every card, the processor and the memory it reports were read.

## Run it

```sh
lokprof --loken 11435 --models qwen3:0.6b --stream
lokprof --ollama 11434 --loken 11435 --models qwen3:0.6b --stream -o run.json
lokprof --ollama 11434 --loken 11435 --models qwen3:0.6b --stream --baseline run.json
```

The first command measures 1 server.  
The second compares 2 servers and saves the record.  
The third measures again and prints the change of each cell against the saved record.

## What it prints

lokprof writes plain text to the terminal, in the formats below.
The values in angle brackets stand for the figures of a run.

<!-- site: widget carousel id=lokprof-overview-outputs -->
```
  ===== LOKEN | model=qwen3:0.6b | num_ctx=4096 | prompt=short =====
  [LOKEN] Warmup streaming (1)... #1 <rate> tok/s
  [LOKEN] Running 3 streaming iterations...
    #1: <rate> tok/s, TTFT <ms>ms, <n> tokens, ITL[p50 <ms>ms p95 <ms>ms], <ms>ms total
        > <the first 120 characters of the answer>
        energy: <J> J (gpu+cpu_pkg+dram) -> <J> J/tok [gpu <J> + cpu <J>], decode <J> J/tok, <g> gCO2/tok
```

**Cell.** Each iteration prints its rates, the start of its answer and its energy.

---

```
  ------------------------------------------------------------------------
  Metric                       Mean     StdDev        Min        Max        P95
  ------------------------------------------------------------------------
  TTFT                         <ms>    +/-<ms>       <ms>       <ms>        n=3
  Completion tok/s           <rate>  +/-<rate>     <rate>     <rate>        n=3
  E2E latency                  <ms>    +/-<ms>       <ms>       <ms>        n=3
  Energy J/tok [gpu+cpu_pkg+dram]        <J>     +/-<J>        <J>        <J>        n=3
  ========================================================================
```

**Results.** Each cell gets the mean, the spread and the extremes of every metric.

---

```
  Comparison: Ollama vs LOKEN  (medians)
  --------------------------------------------------------------------------------
  Metric                           Ollama            LOKEN       Diff
  --------------------------------------------------------------------------------
  TTFT                         <ms> (n=3)       <ms> (n=3)       <x>%
  Completion tok/s           <rate> (n=3)     <rate> (n=3)       <x>%
  Tokens generated              <n> (n=3)        <n> (n=3)          =
```

**Comparison.** Each pair of targets is compared cell by cell, on the medians.

---

```
=== Baseline comparison vs before.json ===
  + [LOKEN|qwen3:0.6b|4096|short]  <rate> -> <rate> tok/s  (+<x>%)
  - [LOKEN|qwen3:0.6b|4096|medium]  <rate> -> <rate> tok/s  (-<x>%)
```

**Baseline.** `--baseline` prints the change of the decode rate of each cell against a saved record.

---

```
  [LOKEN] Running 8 streaming iterations...
    [LOKEN] flight 1/2: 4/4 served, <n> tokens in <s>s -> <rate> tok/s aggregate
    [LOKEN] flight 2/2: 4/4 served, <n> tokens in <s>s -> <rate> tok/s aggregate
```

**Concurrency.** `--concurrency` prints the rate that the server served in each flight of requests.
<!-- site: end -->

## What it does

<!-- site: widget table id=lokprof-overview-capabilities -->
| Capability | Interface | State |
|---|---|---|
| Sweep of models, context lengths and prompts | `--models`, `--num-ctx`, `--prompts` | Ollama, loken and vLLM |
| Time to first token, prefill and decode rates, inter-token latency | `--stream` | Ollama, loken and vLLM |
| Decode rate as the server reports it, beside the client's | `server_decode_tok_s` in the record | Ollama and loken |
| Fresh prompt for every iteration, so no prefix cache answers | `--unique-prompt` | every engine |
| Coherence gate on every answer, with expected words | `--require-substr` | text and vision |
| Energy per token of the cards, the processor and the memory | on by default, [guide](../guides/measure-energy.md) | processor on Linux only |
| Energy of the decode phase apart from the prefill | `--stream` | needs a card read by a counter or through hwmon |
| Idle draw of the machine | `--idle-energy-secs` | recorded, never subtracted |
| Card memory, load and power during each request | on by default, `--no-gpu-sample` | NVIDIA cards |
| Memory and processor use of the engine processes | `--host-proc-names` | Linux only |
| Vision prompts with an attached image | `--image`, [guide](../guides/vision.md) | Ollama and loken |
| Image generation, speech, music and transcription | `--modality`, [guide](../guides/media.md) | no energy figures |
| Concurrent requests | `--concurrency`, [guide](../guides/concurrency.md) | no energy figures |
| Comparison with an earlier record | `--baseline`, [guide](../guides/baseline.md) | decode rate per cell |
| JSON record of the machine, the servers and every iteration | `-o`, [record](../reference/record.md) | schema `loken/2` |
| CSV of every iteration | `--output-csv` | text and vision |
<!-- site: end -->

## What it does not do

lokprof does not start, stop or pin the servers.
Which cards an engine may use is decided when the engine is launched.  
A fair comparison needs each engine alone on the machine, on the same cards, at idle temperature.
The [benchmarks](https://github.com/loken-ai/benchmarks) repository runs that protocol with lokprof, and its [method](https://github.com/loken-ai/benchmarks/blob/main/docs/METHOD.md) describes it.
