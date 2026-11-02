# Find the rate a server serves

Offer requests at a rate of their own and get the highest rate whose latency stays flat.

## Before you start

- [ ] An Ollama, loken or vLLM server.
- [ ] A text model.

## Smallest command

```sh
lokprof --loken 11435 --models qwen3:0.6b --load constant -n 16 --max-tokens 16
```

`--load` takes `constant` or `random`.
`constant` sends a request every `1 / rate` seconds.
`random` draws the gaps from a Poisson process of mean `1 / rate`, the same on every run.  
`-n` sets the requests of each step. A step needs at least 8.

A sequential sweep sends the next request when the last one returns, so a server that slows down is offered less work and its queue never shows.
Here the arrivals are fixed in advance.

## How the rate is found

1. lokprof sends 1 request alone and starts at the rate of that request.
2. A step sends `-n` requests at the rate. It ends when every request has answered.
3. The step is served when the latencies, in arrival order, do not rise. The test is the slope of their least-squares line, one-sided at the 95% level.
4. While steps are served the rate doubles. While they are not it halves.
5. Once a served rate and a refused rate are known, the interval between them is bisected until it is narrower than `1 / sqrt(n)`.

The latency of a request counts from its planned arrival, so a client that falls behind does not hide the queue it makes.

## Expected result

```
  [LOKEN] Offered load, Constant arrivals, 16 requests per step...
    1.42 req/s: median 593 ms, served
    2.84 req/s: median 2052 ms, queue grows
    2.01 req/s: median 620 ms, queue grows
    1.69 req/s: median 548 ms, served
    sustained: 1.69 req/s
```

The results table holds the `Sustained rate` in requests per second.  
The record holds the profile in the settings of the cell.

A failed request is left out of the latencies and named in `iteration_errors` with its rate.

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `offered load needs at least 8 requests per step` | `-n` is below 8. | Raise `-n`. |
| `no rate was served without a rising latency` | Every step showed a rising latency, even the slowest. | Read the steps above it, and check that the server is not shared. |
| `the first request failed` | The request that gives the start rate failed. | Run the same command without `--load`. |
