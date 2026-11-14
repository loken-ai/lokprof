#!/usr/bin/env python3
"""Turn the spans of recorded LLM calls into a session log for `session_to_trace.py`.

Usage: genai_spans_to_session.py SPANS > session.jsonl

SPANS is a file of JSON lines. A line is 1 span, or a batch of spans as the OTLP JSON
encoding writes it under `resourceSpans`. A span is kept when its attributes hold
`gen_ai.input.messages`, as the OpenTelemetry semantic conventions for generative AI define it:

    {"trace_id": "a1", "start_time": "2026-04-20T06:42:30.382312+00:00",
     "attributes": {"gen_ai.input.messages": "[{\"role\": \"user\", \"parts\": [...]}]",
                    "gen_ai.output.messages": "[...]"}}

  trace_id     the session the call belongs to, also read from `context.trace_id`,
               `session_id` or the OTLP `traceId`
  start_time   when the call was sent, an ISO 8601 timestamp or the OTLP
               `startTimeUnixNano`
  attributes   a mapping, or the OTLP list of `key` and `value`

The messages of a call hold `role` and `parts`.
A `text` or `reasoning` part gives its `content`.
A `tool_call` part gives its name and its arguments.
A `tool_call_response` part gives its `response`.
`gen_ai.system_instructions` of the first call opens the session as a system event.

The calls of a session are taken in the order of their start.
The events of a call are the messages its input adds to the input of the call before it,
at the start of the call.
When those messages hold no answer, the output of the call before is put first, at its end.
A call whose input does not extend the input before opens a new session, named after the
first with `#` and its number.
"""

import json
import sys
from datetime import datetime, timezone


def seconds(span, key, otlp_key):
    if key in span:
        return datetime.fromisoformat(span[key].replace("Z", "+00:00")).timestamp()
    if otlp_key in span:
        return int(span[otlp_key]) / 1e9
    return None


def otlp_value(value):
    for kind in ("stringValue", "intValue", "doubleValue", "boolValue"):
        if kind in value:
            return value[kind]
    return value


def attributes(span):
    held = span.get("attributes") or {}
    if isinstance(held, list):
        return {a["key"]: otlp_value(a.get("value", {})) for a in held}
    return held


def trace_id(span):
    for key in ("trace_id", "session_id", "traceId"):
        if span.get(key):
            return str(span[key])
    context = span.get("context") or {}
    if context.get("trace_id"):
        return str(context["trace_id"])
    return None


def messages(value):
    if value is None:
        return None
    if isinstance(value, str):
        value = json.loads(value)
    return value


def part_text(part):
    kind = part.get("type")
    if kind in ("text", "reasoning"):
        return str(part.get("content", ""))
    if kind == "tool_call":
        return f"{part.get('name')}({json.dumps(part.get('arguments'), ensure_ascii=False)})"
    if kind == "tool_call_response":
        response = part.get("response")
        if isinstance(response, str):
            return response
        return json.dumps(response, ensure_ascii=False)
    return json.dumps(part, ensure_ascii=False)


def content(message):
    return "\n".join(part_text(p) for p in message.get("parts", []))


def spans(lines):
    for n, raw in enumerate(lines, 1):
        if not raw.strip():
            continue
        line = json.loads(raw)
        if "resourceSpans" in line:
            for resource in line["resourceSpans"]:
                for scope in resource.get("scopeSpans", []):
                    yield from ((n, s) for s in scope.get("spans", []))
        else:
            yield n, line


def calls(lines):
    out = []
    for n, span in spans(lines):
        attrs = attributes(span)
        given = messages(attrs.get("gen_ai.input.messages"))
        if given is None:
            continue
        start = seconds(span, "start_time", "startTimeUnixNano")
        end = seconds(span, "end_time", "endTimeUnixNano")
        session = trace_id(span)
        if start is None or session is None:
            raise ValueError(f"line {n}: a call needs a start time and a trace id")
        out.append(
            {
                "session": session,
                "start": start,
                "end": end if end is not None else start,
                "input": given,
                "output": messages(attrs.get("gen_ai.output.messages")) or [],
                "system": messages(attrs.get("gen_ai.system_instructions")),
            }
        )
    out.sort(key=lambda c: (c["session"], c["start"]))
    return out


def event(time, session, message):
    return {"time": time, "session": session, "role": message["role"], "content": content(message)}


def convert(lines):
    events = []
    before = {}
    restarts = {}
    for call in calls(lines):
        session = call["session"]
        prev = before.get(session)
        extends = prev is not None and call["input"][: len(prev["input"])] == prev["input"]
        if prev is not None and not extends:
            restarts[session] = restarts.get(session, 0) + 1
        k = restarts.get(session, 0)
        name = session if k == 0 else f"{session}#{k}"
        if extends:
            new = call["input"][len(prev["input"]) :]
            if not any(m["role"] == "assistant" for m in new):
                events += [event(prev["end"], name, m) for m in prev["output"]]
        else:
            new = call["input"]
            if call["system"]:
                text = "\n".join(part_text(p) for p in call["system"])
                events.append({"time": call["start"], "session": name, "role": "system", "content": text})
        events += [event(call["start"], name, m) for m in new]
        before[session] = call
    if not events:
        raise ValueError("the spans hold no call with gen_ai.input.messages")
    # A stable sort keeps the order of the events of 1 call.
    events.sort(key=lambda e: e["time"])
    for e in events:
        e["time"] = datetime.fromtimestamp(e["time"], timezone.utc).isoformat()
    return events


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    with open(sys.argv[1], encoding="utf-8") as f:
        for e in convert(f):
            print(json.dumps(e, ensure_ascii=False))


if __name__ == "__main__":
    main()
