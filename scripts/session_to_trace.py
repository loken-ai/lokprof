#!/usr/bin/env python3
"""Turn the log of an agent session into a trace for `lokprof --replay`.

Usage: session_to_trace.py SESSION_LOG > trace.jsonl

SESSION_LOG is a file of JSON lines, one event per line, in the order they happened:

    {"time": 12.5, "session": "a", "role": "user", "content": "List the files."}

  time     seconds, as a number, or an ISO 8601 timestamp
  session  optional name, the events of 1 conversation share it
  role     user, assistant, system or tool
  content  the text of the event

A request goes out when a user or tool event follows an assistant event, or opens a session.
Its parts are the events since the previous request of the session.
The assistant events are the answers the agent really got, so the replay offers the same
prefix to every engine, whatever each one would have answered.

The offset of a request is its time minus the time of the first request.
Offsets are written in milliseconds, in the order of the log.
The output is the format of the trace that `src/replay.rs` documents.
A tool that logs by session writes this input from the proxy or the agent log of its own.
"""

import json
import sys
from datetime import datetime

TRIGGERS = ("user", "tool")


def seconds(value):
    if isinstance(value, (int, float)):
        return float(value)
    return datetime.fromisoformat(value.replace("Z", "+00:00")).timestamp()


def convert(lines):
    events = []
    for n, raw in enumerate(lines, 1):
        if not raw.strip():
            continue
        e = json.loads(raw)
        for key in ("time", "role", "content"):
            if key not in e:
                raise ValueError(f"line {n}: no {key}")
        events.append(
            (seconds(e["time"]), e.get("session"), e["role"], e["content"], n)
        )
    if not events:
        raise ValueError("the log holds no event")
    pending = {}
    answered = {}
    out = []
    for t, session, role, content, n in events:
        part = pending.setdefault(session, [])
        part.append({"role": role, "content": content})
        # Every part of a session waits for its next trigger.
        if role in TRIGGERS and (answered.get(session, True)):
            line = {"offset_ms": round(t * 1000, 3)}
            if session is not None:
                line["session"] = session
            line["messages"] = part
            out.append(line)
            pending[session] = []
            answered[session] = False
        elif role == "assistant":
            answered[session] = True
    out.sort(key=lambda line: line["offset_ms"])
    first = out[0]["offset_ms"]
    for line in out:
        line["offset_ms"] = round(line["offset_ms"] - first, 3)
    return out


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    with open(sys.argv[1], encoding="utf-8") as f:
        for line in convert(f):
            print(json.dumps(line, ensure_ascii=False))


if __name__ == "__main__":
    main()
