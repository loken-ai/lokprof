# First run

Measure 1 model on 1 server and save the record.

## Before you start

<!-- site: widget checklist id=lokprof-first-run-prereq -->
- [ ] `lokprof` is built, see [Install](install.md).
- [ ] A loken or Ollama server runs on this machine, see [loken First run](https://github.com/loken-ai/loken/blob/main/docs/start/first-run.md).
- [ ] Nothing else runs on the machine while it measures.
<!-- site: end -->

## Run it

<!-- site: widget steps id=lokprof-first-run-steps -->
1. Name the server and the model.
   A bare port means `http://localhost:PORT`.
   A model the server lacks is pulled through its `/api/pull` route first.

   ```sh
   lokprof --loken 11435 --models qwen3:0.6b --prompts short --stream -o first.json
   ```

2. Watch the cell run.
   1 warm-up request is discarded, then 3 iterations are measured.
   Each iteration prints its decode rate, its time to first token, its token count and the start of its answer.

3. Read the results table printed at the end.
   The table gives the mean, the standard deviation, the minimum and the maximum of each metric.
   The P95 column shows `n=` and the sample count below 20 samples.

4. Open `first.json`.
   The record holds the machine, the server, the settings and every iteration, see [Record](../reference/record.md).
<!-- site: end -->

## Expected result

<!-- site: widget replay id=lokprof-first-run-replay -->
```
$ lokprof --loken 11435 --models qwen3:0.6b --prompts short --stream -o first.json

  LLM Benchmark v<version>
  Models:     qwen3:0.6b
  num_ctx:    4096
  Prompts:    short
  Max tokens: 128
  Iterations: 3 (+ 1 warmup)
  Streaming:  yes
  GPU sample: on
  Energy:     on (CI=480 gCO2/kWh)
  LOKEN:      http://localhost:11435

  [LOKEN] Ensuring model 'qwen3:0.6b'... ok
  [LOKEN] Unloading 1 loaded model(s)... ok=1 skip=0
  [LOKEN] Loading model qwen3:0.6b... <ms>ms

  ===== LOKEN | model=qwen3:0.6b | num_ctx=4096 | prompt=short =====
  [LOKEN] Warmup streaming (1)... #1 <rate> tok/s
  [LOKEN] Running 3 streaming iterations...
    #1: <rate> tok/s, TTFT <ms>ms, <n> tokens, ITL[p50 <ms>ms p95 <ms>ms], <ms>ms total
        > <the first 120 characters of the answer>
        energy: not measurable, cpu_pkg could not be read
    #2: <rate> tok/s, TTFT <ms>ms, <n> tokens, ITL[p50 <ms>ms p95 <ms>ms], <ms>ms total
        > <the first 120 characters of the answer>
        energy: not measurable, cpu_pkg could not be read
    #3: <rate> tok/s, TTFT <ms>ms, <n> tokens, ITL[p50 <ms>ms p95 <ms>ms], <ms>ms total
        > <the first 120 characters of the answer>
        energy: not measurable, cpu_pkg could not be read
    [coherence] PASS (3 iter)
  [LOKEN] Model fingerprint: modified_at=<date> params=<size> quant=<level>

  ========================================================================
  LLM Benchmark Results
  ------------------------------------------------------------------------
  Server:     LOKEN (http://localhost:11435)
  Model:      qwen3:0.6b | num_ctx=4096 | prompt=short
  Prompt:     ~58 chars, max 128 tokens, 3 iterations
  ------------------------------------------------------------------------
  Metric                       Mean     StdDev        Min        Max        P95
  ------------------------------------------------------------------------
  Model load                   <ms>     +/-0ms       <ms>       <ms>        n=1
  TTFT                         <ms>    +/-<ms>       <ms>       <ms>        n=3
  Prompt tok/s               <rate>  +/-<rate>     <rate>     <rate>        n=3
  Completion tok/s           <rate>  +/-<rate>     <rate>     <rate>        n=3
  Decode ms/tok                <ms>    +/-<ms>       <ms>       <ms>        n=3
  E2E latency                  <ms>    +/-<ms>       <ms>       <ms>        n=3
  Tokens generated              <n>     +/-<n>        <n>        <n>        n=3
  ITL (per token)              <ms>    +/-<ms>       <ms>       <ms>       <ms>
  ========================================================================

  Coherence preview: > <the first 120 characters of the answer>
  Coherence gate:    PASS
  Results saved to first.json
```
<!-- site: end -->

The energy line reads `not measurable` until the processor's counters are readable, see [Measure energy](../guides/measure-energy.md).  
The `Model fingerprint` line appears when the server's `/api/show` gives the date or the size of the model.  
A machine with NVIDIA cards also prints the memory, load and power of each card after the table.  
On Linux, a line `Host engine procs` gives the memory and processor use of the engine processes.

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `Error: specify at least one model via --models <a,b,c>` | No model was named. | Pass `--models NAME`. |
| `Error: unknown prompt name '...'` | `--prompts` holds a name lokprof does not know. | Use a name from [Prompts](../reference/prompts.md), or `--prompt TEXT`. |
| `FAILED: Pull request failed: ... (skipping)` after `Ensuring model` | No server answers at that address. | Start the server, or correct the port. |
| `FAILED: Pull failed: ... (skipping)` | The server does not have the model and refused to pull it. | Check the model name on the server's registry. |
| `ERROR -- the streaming client speaks plain http, not https` | `--stream` was given an `https` address for Ollama or loken. | Give an `http` address. |
| `ERROR -- the answer ended before its first token: no rate to record` | The model ended its answer before any token. | Use another prompt or another model. |
| `energy: not measurable, cpu_pkg could not be read` | The processor's counters are readable by root only. | Make them readable, see [Measure energy](../guides/measure-energy.md). |
| `[coherence] FAIL - the answer repeats itself` | The answer is a loop, not a text. | Treat the rates of the cell as empty, and check the model. |
| `[vram-wait] GPU0 still at ...MB after 30s - proceeding anyway` | Another process holds memory on card 0. | Stop that process before measuring. |
