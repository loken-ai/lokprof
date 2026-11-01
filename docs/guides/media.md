# Benchmark image, speech and music models

Measure the latency and the throughput of a model that generates an image, speech or music, or that transcribes audio.

## Before you start

<!-- site: widget checklist id=lokprof-media-prereq -->
- [ ] A server that serves the model on the OpenAI route of its kind, listed below.
- [ ] For a transcription: a WAV file, and its exact transcript to score the answer.
<!-- site: end -->

## Modalities

`--modality` tells lokprof what the models are.
lokprof never guesses it from the model name.

| `--modality` | Route | lokprof sends | Rows of the results table |
|---|---|---|---|
| `text`, the default | see [Requests](../reference/requests.md) | the prompt to continue | the text rows |
| `vision` | as `text` | the prompt, and the image of `--image` | the text rows |
| `image` | `/v1/images/generations` | the prompt, a size of 1024x1024, seed 42 | `Image latency`, `Megapixels/s` |
| `tts` | `/v1/audio/speech` | the prompt as the input, voice `alloy`, format `wav` | `TTS latency`, `Real-time factor`, `Audio seconds` |
| `music` | `/v1/audio/generations` | the prompt, a 5 second clip, seed 0 | `Music latency`, `Real-time factor`, `Audio seconds` |
| `asr` | `/v1/audio/transcriptions` | the file of `--audio` | `ASR latency`, `Real-time factor`, `Word error rate`, `Audio seconds` |

`imagegen`, `speech`, `sfx` and `transcribe` are accepted as other names.
An unknown value is measured as `text`.

The real-time factor is the seconds of audio over the seconds of the request.
Above 1, the model is faster than real time.

## Smallest command

<!-- site: widget tabs group=modality id=lokprof-media-commands -->
### Image

```sh
lokprof --loken 11435 --modality image --models MODEL --prompt "A lighthouse at dusk"
```

### Speech

```sh
lokprof --loken 11435 --modality tts --models MODEL --prompt "The quick brown fox jumps over the lazy dog."
```

### Music

```sh
lokprof --loken 11435 --modality music --models MODEL --prompt "A slow piano melody"
```

### Transcription

```sh
lokprof --loken 11435 --modality asr --models MODEL --prompts short \
    --audio sample.wav --reference "the exact words spoken in the sample"
```

A transcription ignores the prompt text, so 1 prompt name gives 1 cell.  
`--reference` takes the transcript itself, not a file.
Without it, the cell reports no word error rate.
<!-- site: end -->

`--prompt` replaces the built-in prompts with your text, as 1 cell named `custom`.

## Expected result

```
  ===== LOKEN | model=MODEL | num_ctx=4096 | prompt=custom =====
  [LOKEN] Image generation, 3 iteration(s)...
    #1: <ms> ms, <x> MP/s
    #2: <ms> ms, <x> MP/s
    #3: <ms> ms, <x> MP/s
```

Speech and music print `<ms> ms, <s>s audio, RTF <x>` for each iteration.
A transcription prints `<ms> ms, RTF <x>, WER <x>`.

Each kind has its own coherence gate:

| Modality | The cell passes when |
|---|---|
| `image` | every iteration produced the same bytes, since the seed is fixed |
| `tts`, `music` | 1 iteration at least returned more than 0.05 seconds of audio |
| `asr` | 1 iteration at least returned a transcript |

Image, speech, music and transcription cells record no energy, no card samples and no per-iteration rows.  
`--stream`, `--max-tokens`, `--unique-prompt` and `--concurrency` do not apply to them.  
In a comparison, the `Diff` of `Megapixels/s` and `Real-time factor` reads a lower value as better, see [Metrics](../reference/metrics.md#comparison-table).

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `asr modality requires --audio` as the error of the cell | `--modality asr` without `--audio`. | Pass `--audio FILE.wav`. |
| `Error: failed to read --audio ...` | The file cannot be read. | Correct the path. |
| `Real-time factor` of 0 on a transcription | The audio is not a WAV, so its length is unknown. | Convert the sample to WAV. |
| `reply is not a parseable WAV` | The speech route answered with another format. | Serve the model with WAV output. |
| `no image in reply: ...` | The image route answered without `data[0].b64_json`. | Check that the server serves the model as an image model. |
| `[coherence] FAIL - a fixed seed produced different images across iterations` | The pipeline is not deterministic at a fixed seed. | Check the pipeline of the model on the server. |
| `[coherence] FAIL - the audio was empty or near-silent` | The model returned silence. | Check the model and the prompt. |
