# Nextest Integration

Nextdeck works with an existing
[`cargo-nextest`](https://nexte.st/) setup. It does not require a Nextdeck-specific setup.

## Setup

Install `cargo-nextest`, then confirm the workspace runs successfully outside
Nextdeck:

```sh
cargo nextest run
```

Open Nextdeck from the Cargo workspace or one of its subdirectories. It will
discover the workspace's tests and make Nextest's run options available in the
interface.

## Configuration

Keep repository-wide test behavior in Nextest's standard configuration file:

```text
.config/nextest.toml
```

Nextdeck makes configured profiles and their `default-filter` values available
when preparing a run. Retries, timeouts, test groups, per-test overrides, and
other settings continue to work as defined by Nextest.

See Nextest's
[repository configuration](https://nexte.st/docs/configuration/) documentation
for the complete format and supported settings.

## Source References

- [Nextest integration](../../src/nextest.rs)
- [Nextest integration tests](../../src/nextest/tests.rs)

## Troubleshooting

If tests are missing or fail before they start, run `cargo nextest run` from
the same workspace first. Resolving the reported Nextest or Cargo error should
also resolve the problem in Nextdeck.
