# Roadmap

The next work. [`CHANGELOG.md`](CHANGELOG.md) lists what shipped.

A row stays until its `Done when` is true.

| State | Meaning |
|---|---|
| Started | Work is under way |
| Partial | Part shipped, the rest is open |
| Planned | Not started |

| Feature | Category | Done when | State |
|---|---|---|---|
| Running format in the record | Records | A record names the format the server runs the model in, read from `details.running_format` of `/api/ps`. A test reads it from the response shape loken sends. | Planned |
