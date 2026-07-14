# Nextdeck

A TUI wrapper for [`cargo-nextest`](https://nexte.st/), Clap-powered xtasks, and more.

## Install

Nextdeck expects `cargo-nextest` to be available in `PATH`.

To install both:

```sh
cargo install --locked cargo-nextest
cargo install --git https://github.com/romansky/nextdeck --locked nextdeck
```

Run Nextdeck from a Cargo workspace:

```sh
nextdeck
```

Or point it to a specific workspace:

```sh
nextdeck --working-dir ../my-workspace
```

This is Nextdeck after running its own test suite at 120x30.

```text
┌ Tests <filters: [p]ass:✓ [f]ail:✓ [i]gnore:✓ ┐┌ Info ────────────────────────────────────────────────────────────────┐
│v [   3.060s] /Users/user/nextdeck •          ││Latest Nextest Run                         Storage                    │
│  v [   3.060s] nextdeck •                    ││run id       1c541c5b-1955-41f7-a79a-86a...status    healthy          │
│    > [   0.264s] app •                       ││status       idle                          available 57.0 GiB         │
│    > [   0.120s] command                     ││result       passed                        updated   2026-07-13 15:...│
│    > [   0.055s] config                      ││profile      default                       /target   8.9 GiB          │
│    > [   0.050s] diagnostics                 ││scope        workspace                                                │
│    > [   0.043s] disk_usage                  ││duration     wall:3.350s aggregate:15.21...                           │
│    > [   0.031s] editor                      ││latest event warn dogfood-output: stderr...                           │
│    > [   0.031s] field_schema                ││progress     348/348                                                  │
│    > [   0.029s] git_status                  ││                                                                      │
│    > [   0.034s] input_field                 │└──────────────────────────────────────────────────────────────────────┘
│    > [   2.626s] nextest •                   │┌ Output <#1-8/8> [s]nap-bottom:✓ ─────────────────────────────────────┐
│    v [   0.136s] output •                    ││DOGFOOD_OUTPUT stdout before info event                               │
│      v [   0.136s] tests •                   ││@ event info dogfood-output: stdout reached info event                │
│          [   0.028s] adjacent_text_chunks_ren││DOGFOOD_OUTPUT stdout after info event                                │
│          [   0.027s] bounded_text_keeps_tail_││DOGFOOD_OUTPUT stderr before warn event                               │
│          [   0.023s] bounded_text_uses_marker││@ event warn dogfood-output: stderr reached warn event                │
│          [   0.022s] captured_text_shows_stdo││DOGFOOD_OUTPUT stdout after warn event                                │
│          [   0.022s] display_text_keeps_runne││                                                                      │
│          [   0.022s] dogfood_output_captures_││Run passed: 348 passed, 0 skipped, 0 ignored                          │
│          [   0.027s] failed_output_has_a_sepa││                                                                      │
│          [   0.027s] interleaved_entries_shar││                                                                      │
│          [   0.031s] interleaved_retention_tr││                                                                      │
│          [   0.037s] late_captured_output_sta││                                                                      │
│          [   0.027s] late_output_adds_separat││                                                                      │
│          [   0.032s] stream_interleaves_text_││                                                                      │
│    > [   0.069s] output_pane                 ││                                                                      │
└ [r]un [j/J]failure [o]pen-editor [u]pdate ───┘└ [/]search<[            ]> [o]pen-editor ─────────────────────────────┘
[Tab]focus [Shift+Left/[]narrow [Shift+Right/]]widen [X]tasks [E]vents [,]settings [D]isk-cleanup [Q]uit
 branch main | unstaged 0:0 | staged 0:0 | tests: idle | storage healthy | key - | Tests pane width: 40%
```

## Features

- **Tests:** Browse tests tree by package, target, module, and name. Run from any selected scope. The output pane
  follows the current focused selection.
- **Custom runs:** Make use of Nextest profiles, filtersets, ignored tests, retries, fail-fast behavior, debugging, and
  stress runs.
- **Xtasks:** Discover and run project-local [`xtask`](https://github.com/matklad/cargo-xtask) style custom commands.
  Arguments appear as editable fields in the TUI.
- **Test events:** Show structured test-time events log alongside captured stdout and stderr, without touching
  application-wide log levels.
- **Long-running tests:** (*macOS only) sample representative stacks, CPU usage, memory, processes, and threads for the
  selected running test.
- **Storage:** Keep an eye on usage and available disk space.

## Integrations

Nextest support works out of the box and requires no code changes. Xtask and test-time events are optional
integrations.

- [Nextest integration guide](docs/nextest-integration/README.md)
- [Clap-powered xtasks integration guide](docs/xtask-integration/README.md)
- [Test events integration guide](docs/test-events/README.md)

The tiny [`nextdeck-helper`](https://docs.rs/nextdeck-helper) crate can be used for quick xtask and test-event
integration.

## Troubleshooting

See the [troubleshooting guide](docs/troubleshooting/README.md).

## Contributing

Bug fixes, reproductions, and tests are welcome as pull requests. For new features, please open an issue so the
scope can be discussed before jumping into code.

## License

Nextdeck is licensed under the [Apache License 2.0](LICENSE).
