# Metrics

The rows of the results table of a cell, the statistics of each row, and the tables printed beside it.
The record keeps each row under `stats`, by its label, see [Record](record.md#cell).

## Text and vision rows

<!-- site: widget table id=lokprof-metrics-text -->
| Row | Unit | Measured as | Present |
|---|---|---|---|
| `Model load` | ms | the load time the server reports for the load request, or the wall time of that request | Ollama and loken, 1 value per cell |
| `TTFT` | ms | from the request to the first chunk that carries text, on the client's clock | with `--stream` |
| `Prompt tok/s` | tok/s | `prompt_eval_count` over `prompt_eval_duration`, as the server reports them | Ollama and loken |
| `Completion tok/s` | tok/s | with `--stream`, the tokens after the first over the time from the first token to the last. Without it, the tokens over the whole request | always |
| `Decode ms/tok` | ms | 1000 over `Completion tok/s` | with `--stream` |
| `E2E latency` | ms | the wall time of the request, on the client's clock | always |
| `Tokens generated` | count | the tokens the server reports, `eval_count` or `completion_tokens`, or else the chunks that carried text | always |
| `ITL (per token)` | ms | the gaps between chunks that carry text, pooled over every iteration | with `--stream` |
| `Energy J/tok [DOMAINS]` | J | the energy of the window over the tokens generated | complete windows |
| `Energy decode J/tok [DOMAINS]` | J | the energy from the first token to the end of the window, over the tokens generated | complete windows, with `--stream` and a card read by a counter or through hwmon |
| `Energy Wh/tok [DOMAINS]` | Wh | `Energy J/tok` over 3600 | complete windows |
| `Carbon gCO2/tok [DOMAINS]` | g | `Energy Wh/tok` times `--carbon-intensity`, over 1000 | complete windows |
<!-- site: end -->

`DOMAINS` names what the energy counts, as `gpu+cpu_pkg+dram`.
2 rows with different domains measure different things, and the comparison table does not pair them.

A streamed answer of 1 token has no decode interval.
Its `Completion tok/s` falls back to `eval_count` over `eval_duration` as the server reports them, and is absent on vLLM.  
The record also keeps that server rate for every Ollama and loken iteration, as `server_decode_tok_s`.

## Image, speech, music and transcription rows

<!-- site: widget table id=lokprof-metrics-media -->
| Row | Unit | Measured as | Modality |
|---|---|---|---|
| `Image latency` | ms | the wall time of the request | `image` |
| `Megapixels/s` | MP/s | the pixels asked for, 1024 x 1024, over the wall time | `image` |
| `TTS latency` | ms | the wall time of the request | `tts` |
| `Music latency` | ms | the wall time of the request | `music` |
| `ASR latency` | ms | the wall time of the request | `asr` |
| `Real-time factor` | x | the seconds of audio over the seconds of the request | `tts`, `music`, `asr` |
| `Audio seconds` | s | the length of the audio, read from the WAV header | `tts`, `music`, `asr` |
| `Word error rate` | wer | the word edit distance to `--reference`, over the words of the reference, after lower case and without punctuation | `asr` with `--reference` |
<!-- site: end -->

## Statistics

| Statistic | Computed as |
|---|---|
| `mean` | the mean of the values |
| `stddev` | the sample standard deviation, divided by n minus 1 |
| `min`, `max` | the smallest and the largest value |
| `p50`, `p95` | the percentile, interpolated between the 2 nearest values |
| `count` | the values kept |

A value that is not a finite number is left out of the minimum, the maximum, the percentiles and the count.  
In the results table, the `P95` column shows `n=` and the count below 20 values.  
A time of 1000 ms or more prints in seconds.

## Comparison table

The comparison of 2 targets reads the median of each row.

| Row | `Diff` |
|---|---|
| unit `tok/s` | the second rate over the first, minus 1, in percent |
| unit `count` | `=` when equal, `differ` otherwise |
| every other unit | the first value over the second, minus 1, in percent |
| absent on the second target | `-` |

A positive `Diff` means the second target is better.
Every row other than `tok/s` reads a lower value as better, `Megapixels/s` and `Real-time factor` included.

## Card and host tables

After the results table, a machine with NVIDIA cards prints 1 line per card:

| Column | Value |
|---|---|
| `VRAM peak`, `VRAM avg` | the memory used on the card by every process, in MB |
| `Util pk` | the peak load of the card, in percent |
| `Power pk` | the peak power of the card, in watts |

On Linux, the `Host engine procs` line sums the processes named by `--host-proc-names`:

| Column | Value |
|---|---|
| `RSS pk`, `avg` | the resident memory, swapped pages included, in MB |
| `swap pk` | the swapped memory, in MB |
| `CPU avg`, `pk` | the processor use, where 100% is 1 core |

The 2 tables print in this form, the values in angle brackets standing for the figures of a run:

```
  GPU usage during this cell:
  GPU    Name                            VRAM peak     VRAM avg    Util pk   Power pk
  0      <card name>                         <n>MB        <n>MB       <n>%      <n>W

  Host   engine procs                 RSS pk    <n> MB  avg    <n> MB  swap pk   <n> MB  CPU avg  <n>%  pk  <n>%
```
