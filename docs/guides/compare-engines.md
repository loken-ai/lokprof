# Compare 2 servers

Measure the same models on Ollama, loken or vLLM, and get a comparison table for every cell.

## Before you start

<!-- site: widget checklist id=lokprof-compare-prereq -->
- [ ] Each server runs on its own port.
- [ ] Each server can serve the models under the same name, since every target receives the names of `--models`.
- [ ] A vLLM server was launched with the model, under that name.
- [ ] The machine runs nothing else.
<!-- site: end -->

## Smallest command

```sh
lokprof --ollama 11434 --loken 11435 --models qwen3:0.6b --stream
```

## Variants

<!-- site: widget tabs group=targets id=lokprof-compare-variants -->
### Ollama and loken

```sh
lokprof --ollama 11434 --loken 11435 --models qwen3:0.6b,qwen3:1.7b \
    --num-ctx 4096,8192 --stream --unique-prompt -o compare.json
```

Both servers load and unload their models through `/api/generate`.

### With vLLM

```sh
lokprof --ollama 11434 --loken 11435 --vllm 8000 --models MODEL --stream -o compare.json
```

vLLM serves 1 model, fixed at its launch.
`--num-ctx` does not reach it, and its window is the one set at its launch.  
Without `--stream`, lokprof warns that the vLLM rate includes the prefill.

### On the processor

```sh
lokprof --ollama 11434 --loken 11435 --models qwen3:0.6b --stream --num-gpu 0
```

`--num-gpu 0` adds `num_gpu: 0` to the options of every Ollama and loken request.
Ollama then keeps every layer on the processor, and loken ignores the field.  
Launch the other engines on the processor yourself, or the run compares a processor with a card.
<!-- site: end -->

## Run it

<!-- site: widget steps id=lokprof-compare-steps -->
1. Run the sweep with `--stream`, so every engine gets a time to first token and a decode rate.
2. Add `--unique-prompt` when the prefill is the point.
   Every iteration then starts with a counter, so no prefix cache answers it.
3. Save the record with `-o`, to compare it later with `--baseline`.
<!-- site: end -->

## Expected result

After the tables of the cells, lokprof prints 1 comparison per pair of targets, cell by cell:

```
  +======================================================================+
  | SWEEP COMPARISON (Ollama vs LOKEN)
  +======================================================================+

  --- model=qwen3:0.6b | num_ctx=4096 | prompt=short ---

  ================================================================================
  Comparison: Ollama vs LOKEN  (medians)
  --------------------------------------------------------------------------------
  Metric                           Ollama            LOKEN       Diff
  --------------------------------------------------------------------------------
  Model load                   <ms> (n=1)       <ms> (n=1)       <x>%
  TTFT                         <ms> (n=3)       <ms> (n=3)       <x>%
  Prompt tok/s               <rate> (n=3)     <rate> (n=3)       <x>%
  Completion tok/s           <rate> (n=3)     <rate> (n=3)       <x>%
  Decode ms/tok                <ms> (n=3)       <ms> (n=3)       <x>%
  E2E latency                  <ms> (n=3)       <ms> (n=3)       <x>%
  Tokens generated              <n> (n=3)        <n> (n=3)          =
  ITL (per token)            <ms> (n=<n>)     <ms> (n=<n>)       <x>%
  ================================================================================
```

The comparison reads the medians.
A positive `Diff` means the second target is better: faster for a rate, shorter or lower for every other metric.  
`Tokens generated` shows `=` or `differ`, never a percentage.
2 cells with different token counts did different work, and their rates do not compare.  
The pairs follow the order Ollama, loken, vLLM, whatever the order of the options.

`protocol.engines_seen` in the record lists the engine processes running when the record was written.
A name there beside the target says that another engine shared the machine.

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `vLLM target at ... is not serving '...'` | No vLLM server answers at that address, or it lists no model. | Start vLLM with the model, or correct the port. |
| `vLLM target without --stream: decode tok/s will include prefill time` | The vLLM API reports no prefill and decode split. | Add `--stream`. |
| `Stream failed: ...` or `Completion failed: ...` on vLLM | vLLM refused the request, for example a model name it does not serve. | Pass the name that vLLM serves to `--models`. |
| `[vram-wait] GPU0 still at ...MB after 30s - proceeding anyway` | A model or a process still holds memory on card 0 before the next model loads. | Stop the process that holds it. |
