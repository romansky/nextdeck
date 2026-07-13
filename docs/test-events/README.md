# Test Events

Captured stdout is useful after a failure, but it has no structure. Test events
add small, searchable records for the decisions that explain a test: which
fixture was selected, whether a cache hit, or which external service handled a
request.

Events are optional. The macro is a no-op unless `NEXTDECK_TEST_EVENTS` is set;
Nextdeck sets it for tests that it launches.

## Add Events to a Test

Add the helper as a development dependency:

```toml
[dev-dependencies]
nextdeck-test-events = "0.1"
```

Emit an event from a test or helper called by a test:

```rust
#[test]
fn reuses_the_cached_artifact() {
    let cache_key = "docs-v2";

    nextdeck_test_events::event!(
        level: nextdeck_test_events::Level::Info,
        target: "artifact-cache",
        "cache hit";
        "key" => cache_key,
        "source" => "local",
    );

    // Continue the test.
}
```

The short form emits an `info` event and uses the Rust module path as its
target:

```rust
nextdeck_test_events::event!("fixture ready"; "rows" => rows.len());
```

Field values can be any type that implements `serde::Serialize`.

## View Events

Run the test from Nextdeck. Nextdeck sets `NEXTDECK_TEST_EVENTS` for the test
process and tails the resulting event files while nextest is running.

Running this repository produces four events. This shortened capture shows the
run they belong to, their fields, and the source line that emitted them:

```text
┌ Test Events ───────────────────────────────────────────────────────────────────────────────────────┐
│┌ Runs ──────────────────────────┐ ┌ Events <#1-25/25> [s]nap-bottom:✓ ────────────────────────────┐│
││> passed      4 workspace       │ │#1  info  nextdeck::app::tests     verifying inline test event││
││                                │ │    { "component": "app" }                                  ││
││                                │ │    at nextdeck::app::tests src/app/tests.rs:438              ││
││                                │ │                                                               ││
││                                │ │#2  info  nextdeck::nextest::tests running nextdeck fixture   ││
││                                │ │    { "fixture": "pass_emits_nextdeck_event" }              ││
││                                │ │    at nextdeck::nextest::tests src/nextest/tests.rs:113      ││
││                                │ │                                                               ││
││                                │ │#3  info  dogfood-output  stdout reached info event           ││
││                                │ │    { "step": 1, "stream": "stdout" }                     ││
││                                │ │    at nextdeck::output::tests src/output/tests.rs:98         ││
││                                │ │                                                               ││
││                                │ │#4  warn  dogfood-output  stderr reached warn event           ││
│└ [tab]events ───────────────────┘ └ [/]search<[            ]> [o]pen-editor ──────────────────────┘│
└ [esc]close [tab]events ────────────────────────────────────────────────────────────────────────────┘
```

- Events appear inline with captured output when their thread name can be
  matched to a test.
- Press `E` for the event view, which keeps a searchable stream for each run.
- The event view records the run scope and final status as well as the events.

Nextdeck creates a separate JSONL file per process. This avoids coordinating
writes between the test binaries that nextest runs in parallel.

## What Gets Recorded

The event macro records a schema version, timestamp, process ID, level,
message, and source location. A thread name, target, and extra fields are
included when available. Events built directly with `TestEvent` can omit the
optional source, thread, target, and fields.

```json
{
  "schema_version": 1,
  "time": 1783420000000,
  "pid": 12345,
  "thread": "tests::reuses_the_cached_artifact",
  "level": "info",
  "target": "artifact-cache",
  "message": "cache hit",
  "fields": {
    "key": "docs-v2",
    "source": "local"
  },
  "source": {
    "module": "my_crate::tests",
    "file": "src/lib.rs",
    "line": 42
  }
}
```

The macro intentionally ignores write errors so instrumentation cannot fail a
test. Code that needs error handling can build a `TestEvent` and call
`nextdeck_test_events::emit`, which returns `std::io::Result<()>`.

## Scope

Test events are not a general logging framework and do not replace `tracing` or
`log`. They are a narrow side channel for data that is most useful while
examining a test run. The environment-variable gate means normal `cargo test`
and `cargo nextest run` invocations do not create event files.

The same helper crate also provides the optional
[Clap xtask integration](../xtask-integration/README.md).
