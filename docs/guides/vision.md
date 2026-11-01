# Benchmark a vision model

Measure a model that answers about an image, with the same rates as a text cell and a check on what it describes.

## Before you start

<!-- site: widget checklist id=lokprof-vision-prereq -->
- [ ] An Ollama or loken server with a vision model.
- [ ] An image file, sent as it is.
- [ ] A few words that a correct description of the image contains.
<!-- site: end -->

vLLM is measured on its text route only, and refuses every vision iteration.

## Smallest command

```sh
lokprof --loken 11435 --models MODEL --image photo.jpg \
    --prompts vision_short,vision_medium,vision_long --stream
```

lokprof reads the image once, encodes it in base64 and attaches it to every request in `images`.
The vision prompts ask for a description at 3 lengths, see [Prompts](../reference/prompts.md).  
A vision request keeps the model's chat template, unlike a text request.

## Run it

<!-- site: widget steps id=lokprof-vision-steps -->
1. Run the smallest command.
   Each iteration pays the prefill of the image, and lokprof warns `Vision run without --session-id` before the sweep.
2. Add `--require-substr` with words the description must hold, separated by commas.
   An iteration whose first 120 characters hold none of them fails the cell.

   ```sh
   lokprof --loken 11435 --models MODEL --image photo.jpg \
       --prompts vision_short --stream --require-substr dog,puppy
   ```

3. Add `--session-id NAME` to measure a server that reuses the image across requests.
   Every request then carries `session_id` in its options.
   loken reuses the computed prefix under that id, and Ollama ignores the field.
<!-- site: end -->

`--session-id` changes what a cell measures on one engine and not on the other.
The record keeps it in `config.session_id` and `protocol.prefix_reuse_offered`.

## Expected result

```
  Image:      photo.jpg (<n> bytes raw, <n> bytes b64)
  ...
    [coherence] PASS (3 iter, 2 substring gate)
```

The results table holds the rows of a text cell, see [Metrics](../reference/metrics.md).

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `Error: failed to read --image ...` | The file cannot be read. | Correct the path. |
| `vLLM vision benchmarking via --vllm is not supported` in the errors of the cell | vLLM is measured on `/v1/completions`, a text route. | Measure the vision model on Ollama or loken. |
| `[coherence] FAIL - an iteration contained none of the required substrings` | An answer held none of the words in its first 120 characters. | Read the preview of the cell, then check the model or the words. |
| The answers ignore the image | `--image` was not given, and the vision prompts ran on text alone. | Add `--image`. |
