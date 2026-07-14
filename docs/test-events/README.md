# Test Events

Nextdeck can help facilitate test-time event logs, these can be generated and captureed regardlress of the log level or
configuration of the apps logging framework, test events are accessible via "E" global command.

## Rust Setup

Add the helper as a development dependency:

```toml
[dev-dependencies]
nextdeck-helper = "0.1"
```

Emit an event from anywhere in your code:

```rust
nextdeck_helper::event!(
    level: nextdeck_helper::Level::Info,
    target: "artifact-cache",
    "cache hit";
    "key" => "docs-v2",
    "source" => "local",
);
```

The short form emits an `info` event and uses the Rust module path as its target:

```rust
nextdeck_helper::event!("fixture ready"; "rows" => rows.len());
```

Field values may be any type implementing `serde::Serialize`.

The `nextdeck_helper::event` macro enter and emit only when `NEXTDECK_TEST_EVENTS` environment variable is present, it
is set by Nextdeck automatically when its running tests.

## Output Schema

Events are emitted as a single line in the form `NEXTDECK_EVENT_V1 <single-line JSON>`.

The event JSON Schema is represented below as an illustrative TypeScript type:

```ts
type TestEventV1 = {
    schema_version: 1;
    sequence: number; // Unsigned and monotonically increasing per process
    time: number; // Unix time in milliseconds
    level: "trace" | "debug" | "info" | "warn" | "error";
    message: string;

    pid?: number; // Include when events can come from multiple processes
    thread?: string;
    target?: string;
    fields?: Record<string, unknown>;
    source?: {
        module: string;
        file: string;
        line: number;
    };
};
```

## Source References

- [Rust event API and wire-format implementation](https://docs.rs/crate/nextdeck-helper/latest/source/src/lib.rs)
- [Test fixture using the event macro](../../tests/fixtures/output-workspace/src/lib.rs)
- [Nextdeck's event parser](../../src/test_events.rs)
