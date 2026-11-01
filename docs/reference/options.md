# Options

```sh
lokprof [OPTIONS] --models <MODELS> (--ollama <ADDRESS> | --loken <ADDRESS> | --vllm <ADDRESS>)...
```

At least 1 target and 1 model are required.  
A list takes its values separated by commas, as in `--models a,b,c`.

## Addresses

`--ollama`, `--loken` and `--vllm` take an address in any of these forms:

| Given | Used |
|---|---|
| `11435` | `http://localhost:11435` |
| `host:11435` | `http://host:11435` |
| `http://host:11435` or `https://host:11435` | as given, without a trailing `/` |

A streamed request to Ollama or loken speaks plain HTTP and refuses an `https` address.

## Targets and sweep

<!-- site: widget table id=lokprof-options-sweep -->
| Option | Value | Default | Effect |
|---|---|---|---|
| `--ollama` | address | none | Measures an Ollama server, labelled `Ollama` |
| `--loken` | address | none | Measures a loken server, labelled with `--loken-label` |
| `--loken-label` | text | `LOKEN` | Names the loken target in the output and the record |
| `--vllm` | address | none | Measures a vLLM server, labelled `vLLM` |
| `--models` | list | none | The models to measure, the same names on every target |
| `-m`, `--model` | name | none | 1 model, used when `--models` is absent |
| `--num-ctx` | list of integers | `4096` | The context lengths, sent as `num_ctx` to Ollama and loken. vLLM keeps the window set at its launch |
| `--prompts` | list of names | `short,medium,long` | The built-in prompts, see [Prompts](prompts.md) |
| `-p`, `--prompt` | text | none | 1 prompt of your own, named `custom`, in place of `--prompts` |
| `--modality` | `text`, `vision`, `image`, `tts`, `music`, `asr` | `text` | What the models are, see [Benchmark image, speech and music models](../guides/media.md) |
<!-- site: end -->

`--modality` also accepts `imagegen` and `image-gen` for `image`, `speech` for `tts`, `sfx`, `audiogen` and `audio-gen` for `music`, and `transcribe` for `asr`, in any case.
Any other value is measured as `text`.

## Requests

<!-- site: widget table id=lokprof-options-requests -->
| Option | Value | Default | Effect |
|---|---|---|---|
| `--stream` | flag | off | Streams every text request, which gives the time to first token, the decode rate and the decode energy |
| `--max-tokens` | integer | `128` | The tokens each answer may generate |
| `-n`, `--iterations` | integer | `3` | The measured requests of each cell |
| `-w`, `--warmup` | integer | `1` | The requests sent before the iterations of each cell, and discarded |
| `--concurrency` | integer | `1` | The requests in flight at once, see [Measure concurrent requests](../guides/concurrency.md) |
| `--unique-prompt` | flag | off | Starts the prompt of each iteration with `Request N of a benchmark series.` and an empty line, so no prefix cache answers it |
| `--request-timeout` | seconds | `600` | How long a request may take before it is given up. For a stream, how long it may stay silent |
| `--num-gpu` | integer | not sent | Sends `num_gpu` in the options of every Ollama and loken request. Ollama keeps that many layers on its cards, loken ignores it |
| `--session-id` | text | none | Sends `session_id` in the options of every Ollama and loken request. loken reuses the computed prefix under it |
| `--image` | path | none | Attaches the image, in base64, to every Ollama and loken text request |
| `--audio` | path to a WAV | none | The audio of a `--modality asr` cell |
| `--reference` | text | none | The exact transcript of `--audio`, for the word error rate |
| `--require-substr` | list | none | Fails the cell when the first 120 characters of an answer hold none of the words, whatever their case |
<!-- site: end -->

A model missing from an Ollama or loken target is pulled through `/api/pull` before the sweep.  
[Requests](requests.md) lists every route lokprof calls.

## Measurement

<!-- site: widget table id=lokprof-options-measurement -->
| Option | Value | Default | Effect |
|---|---|---|---|
| `--no-energy` | flag | off | Turns off the energy measurement |
| `--idle-energy-secs` | seconds | `0` | Reads the machine with no request for that long before the sweep, and records it as `idle_energy_baseline`. `0` turns it off |
| `--carbon-intensity` | gCO2eq per kWh | `480` | The grid intensity behind `gco2_per_tok` |
| `--no-gpu-sample` | flag | off | Turns off the samples of card memory, load and power |
| `--gpu-sample-interval-ms` | milliseconds | `100` | The period of the card samples and of the energy readings. The host samples use the larger of it and 200 |
| `--host-proc-names` | list | `lokend,ollama,python,vllm` | The process names whose memory and processor use make the host footprint, matched against `/proc/PID/comm` |
<!-- site: end -->

## Output

<!-- site: widget table id=lokprof-options-output -->
| Option | Value | Default | Effect |
|---|---|---|---|
| `-o`, `--output` | path | none | Writes the JSON record, see [Record](record.md) |
| `--output-csv` | path | none | Writes 1 CSV row per iteration, see [Record](record.md#csv-file) |
| `--baseline` | path | none | Compares the decode rate of each cell with an earlier record, see [Compare with an earlier run](../guides/baseline.md) |
| `-v`, `--verbose` | flag | off | Prints the requests and the answers of the servers on the error output |
| `-h`, `--help` | flag | | Prints the options |
| `-V`, `--version` | flag | | Prints the version |
<!-- site: end -->

## Exit status

| Status | When |
|---|---|
| `0` | The sweep ran, whatever the result of its cells. `--help` and `--version` also exit with `0` |
| `1` | No target, no model, an unknown prompt name, or an `--image` or `--audio` file that cannot be read |
| `2` | An unknown option or a value of the wrong type |

A cell that failed, a record that could not be written and a baseline that could not be read print an error and keep the status at `0`.
