# Requests

The routes lokprof calls on each target, in the order of a run.

## Ollama and loken

<!-- site: widget table id=lokprof-requests-ollama -->
| When | Request | Purpose |
|---|---|---|
| Before the sweep | `GET /api/version`, then `GET /version` | What the server says it is, kept in `subjects` |
| Before the sweep | `GET /api/distributed/devices` | The devices and backends the server detected, kept in `subjects`. loken answers it |
| Pre-flight | `POST /api/show` | Whether the server has the model |
| Pre-flight, model missing | `POST /api/pull` with `"stream": true` | Pulls the model, for up to 1 hour |
| Before each model | `GET /api/ps` | The models loaded on the server |
| Before each model | `POST /api/generate` with an empty prompt and `"keep_alive": 0` | Unloads every loaded model and every model of `--models`, on every target |
| Before each model | `POST /api/generate` with an empty prompt and `"keep_alive": "30m"` | Loads the model, and gives its load time |
| Each warm-up and iteration | `POST /api/generate` | The measured request, below |
| After each text cell | `POST /api/show` | The fingerprint of the model: `modified_at`, `parameter_size`, `quantization_level` |
| After the sweep | `POST /api/generate` with `"keep_alive": 0` | Unloads the models measured |
<!-- site: end -->

The probes before the sweep wait 5 seconds at most.  
Before each model on Ollama or loken, lokprof also asks `nvidia-smi` for the memory used on card 0, once a second, until it drops under 2048 MB or 30 seconds pass.
A machine without `nvidia-smi` skips the wait.

The measured request carries:

| Field | Value |
|---|---|
| `model`, `prompt` | the model and the prompt of the cell |
| `stream` | `true` with `--stream` |
| `raw` | `true` for a text prompt, absent when an image is attached |
| `images` | the image of `--image`, in base64 |
| `think`, `thinking` | `false`, so a reasoning model writes its answer in `response` |
| `keep_alive` | `"30m"` |
| `options.num_predict` | `--max-tokens` |
| `options.temperature` | `0` |
| `options.num_ctx` | the context length of the cell |
| `options.num_gpu` | `--num-gpu`, when given |
| `options.session_id` | `--session-id`, when given |

An answer of status 200 that carries an `error` field, or a `response` that starts with `Error generating response:` or `Error: `, counts as a failed request.

## vLLM

<!-- site: widget table id=lokprof-requests-vllm -->
| When | Request | Purpose |
|---|---|---|
| Before the sweep | `GET /api/version`, `GET /version`, `GET /api/distributed/devices` | As for Ollama and loken |
| Pre-flight and load | `GET /v1/models` | Whether the server serves a model |
| Each warm-up and iteration | `POST /v1/completions` | The measured request: `model`, `prompt`, `max_tokens`, `temperature` 0, `stream` |
<!-- site: end -->

A streamed request also asks for `stream_options.include_usage`, so the last chunk carries the token counts.  
lokprof neither loads, unloads nor pulls a vLLM model.

## Image, speech, music and transcription

A cell of `--modality` `image`, `tts`, `music` or `asr` calls the OpenAI route of its kind, on any target.

| `--modality` | Request | Body |
|---|---|---|
| `image` | `POST /v1/images/generations` | `model`, `prompt`, `size` `1024x1024`, `n` 1, `seed` 42, `response_format` `b64_json` |
| `tts` | `POST /v1/audio/speech` | `model`, `input` the prompt, `voice` `alloy`, `response_format` `wav` |
| `music` | `POST /v1/audio/generations` | `model`, `prompt`, `seconds` 5, `seed` 0 |
| `asr` | `POST /v1/audio/transcriptions` | a form with `model`, and `file` the audio of `--audio` as `audio.wav` |

The pre-flight and the load of an Ollama or loken target run before these cells as before a text cell.
