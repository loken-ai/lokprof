# Replay a recorded session

Send the requests of a trace at the offsets it holds, and get the latency of each one.

## Before you start

- [ ] An Ollama, loken or vLLM server.
- [ ] A text model.
- [ ] A trace file, see [Trace file](#trace-file).

## Smallest command

```sh
lokprof --loken 11435 --models qwen3:0.6b --replay session.jsonl --stream
```

`--replay` replaces `--prompt` and `--prompts`.
The cell is named `replay:` and the first 12 digits of the digest of the trace.
`--stream` gives the time to first token and the time per output token.
`--max-tokens` applies to a request that holds no `max_tokens`.
No warm-up request is sent.

Each request is sent at its offset from the start of the replay, whatever the engine has answered.
A late answer never delays the next send.

## Trace file

1 JSON object per line. A blank line is skipped.

| Field | Holds |
|---|---|
| `offset_ms` | the time to send the request, in ms from the start. Required, 0 or more, never below the offset of the line before |
| `prompt` | the text of the request |
| `messages` | a list of `{"role", "content"}`, sent as 1 line `role: content` each. Give `prompt` or `messages`, not both |
| `max_tokens` | the tokens the answer may generate. Optional |
| `session` | a name. Optional |

A field that is not listed is refused, and the error names the line.

Requests of 1 session share a prefix that grows.
The prompt sent for a request is the text of the earlier requests of its session, then its own text, joined by an empty line.
A request with no `session` is sent as it is.
lokprof sends the session name as `session_id` to Ollama and loken.

An example is in `tests/trace.jsonl`.

## Compare 2 engines

The record holds the SHA-256 of the bytes of the file in `config.replay_trace_sha256`.
2 records with the same digest were offered the same requests at the same times.
Changing a byte of the file, an offset included, changes the digest.
