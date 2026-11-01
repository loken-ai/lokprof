# Compare with an earlier run

Measure again and get the change of the decode rate of every cell against a saved record.

## Before you start

<!-- site: widget checklist id=lokprof-baseline-prereq -->
- [ ] A record written by `-o` from an earlier run.
- [ ] The same targets, models, context lengths and prompts as that run.
<!-- site: end -->

## Smallest command

```sh
lokprof --loken 11435 --models qwen3:0.6b --stream -o after.json --baseline before.json
```

lokprof runs the sweep, then reads `before.json` and matches each cell by its label, model, context length and prompt.

## Run it

<!-- site: widget steps id=lokprof-baseline-steps -->
1. Save a first record.

   ```sh
   lokprof --loken 11435 --models qwen3:0.6b --stream -o before.json
   ```

2. Change the server: a new build, a new setting.
3. Run the same sweep with `--baseline before.json`, and `-o` to keep the new record.
<!-- site: end -->

## Expected result

```
=== Baseline comparison vs before.json ===
  = [LOKEN|qwen3:0.6b|4096|short]  <rate> -> <rate> tok/s  (+<x>%)
  + [LOKEN|qwen3:0.6b|4096|medium]  <rate> -> <rate> tok/s  (+<x>%)
  - [LOKEN|qwen3:0.6b|4096|long]  <rate> -> <rate> tok/s  (-<x>%)
```

Each line compares the mean `Completion tok/s` of the cell, the earlier value first.

| Mark | Meaning |
|---|---|
| `+` | faster by more than 1% |
| `=` | within 1% |
| `-` | slower by more than 1% |
| `?` | 1 of the 2 cells has no decode rate |
| `NO BASELINE - skipping` | the earlier record has no such cell |

Each cell also compares the `modified_at` date that the server gave for the model.
When the dates differ, lokprof marks the cell `DRIFT` and ends with a note: the weights changed between the 2 runs, so the change measures other weights, not other code.  
A vLLM cell has no date, so it is never marked.

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `Could not read baseline: ...` | The file cannot be read. | Correct the path. The sweep and its record are kept. |
| `Could not parse baseline JSON: ...` | The file is not JSON. | Pass a record written by `-o`. |
| `Baseline missing 'results' array` | The JSON is not a lokprof record. | Pass a record written by `-o`. |
| Every cell reads `NO BASELINE` | A label, a model, a context length or a prompt differs from the earlier run. | Run the same sweep, with the same `--loken-label`. |
