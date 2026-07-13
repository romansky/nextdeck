# nextdeck

A TUI wrapper for [`cargo-nextest`](https://nexte.st/), Clap-powered xtasks, and more.

## Install

Nextdeck expects `cargo-nextest` to be available on `PATH`.

```sh
cargo install --locked cargo-nextest
cargo install --locked nextdeck
```

Run it from a Cargo workspace:

```sh
nextdeck
```

This is Nextdeck after running its own test suite at 120x24. The home path,
run ID, and machine-specific storage values have been shortened.

```text
┌ Tests <filters: [p]ass:✓ [f┐┌ Info ──────────────────────────────────────────────────────────────────────────────────┐
│v [   2.911s] ~/nextdeck    ││Latest Nextest Run                                     Storage                          │
│  > [   2.911s] nextdeck •  ││run id       87fa9e9f...                               status    healthy                │
│                            ││status       idle                                      available -                      │
│                            ││result       passed                                    updated   -                      │
│                            ││profile      default                                   /target   -                      │
│                            ││scope        workspace                                                                  │
│                            ││duration     wall:3.168s aggregate:14.628s build:0.2...                                 │
│                            ││latest event warn dogfood-output: stderr reached war...                                 │
│                            ││progress     338/338                                                                    │
│                            ││                                                                                        │
│                            │└────────────────────────────────────────────────────────────────────────────────────────┘
│                            │┌ Output <#1863-1871/1871> [s]nap-bottom:✓ ──────────────────────────────────────────────┐
│                            ││  output ───                                                                            │
│                            ││                                                                                        │
│                            ││test xtask::tests::manifest_refresh_drops_values_that_no_longer_match_spec ... ok       │
│                            ││                                                                                        │
│                            ││nextdeck::xtask::tests::omits_optional_defaults [passed]                                │
│                            ││duration: 23.90ms                                                                       │
│                            ││                                                                                        │
│                            ││                                                                                        │
└ [enter]details [r]un [R]run┘└ [/]search<[            ]> [o]pen-editor ───────────────────────────────────────────────┘
─ [Tab]focus [Shift+[/Left]narrow [Shift+]/Right]widen [X]tasks [E]vents [,]settings [D]isk-cleanup [Q]uit ─────────────
 branch main | run idle | storage healthy | key - | Passed: 338 passed, 0 skipped
```

## What it does

- Discovers tests through `cargo nextest list` and groups them by package,
  target, module, and test.
- Runs a test, module, target, package, the workspace, or the tests that failed
  in the previous run.
- Exposes nextest profiles, filtersets, retries, ignored tests, fail-fast,
  debugger, and stress options in a custom-run form.
- Captures test output and keeps it attached to the test that produced it.
- Discovers project-local `cargo xtask` commands and renders supported Clap
  arguments as controls.
- Shows events emitted from tests alongside their normal output.

## First run

Move through the test tree with the arrow keys and press `r` to run the
selected scope. Press `R` for the custom-run form.

| Key | Action |
| --- | --- |
| `r` | Run the selected scope |
| `R` | Configure a custom run |
| `Enter` | Open details for the selected item |
| `Tab` | Switch between the test tree and output |
| `j` / `J` | Jump to the next or previous failure |
| `o` | Open the selected test or output in an editor |
| `x` | Open project xtasks |
| `e` | Open test events |
| `?` | Show all keys for the current view |
| `q` | Quit |

The help view is context-sensitive and is the best reference for keys outside
the main test view.

## Integrations

Nextdeck works with an existing nextest setup and reads the workspace's
`.config/nextest.toml`.

- [Nextest integration](docs/nextest-integration/README.md)
- [Clap xtask integration](docs/xtask-integration/README.md)
- [Test events](docs/test-events/README.md)

Both optional integrations use the small
[`nextdeck-test-events`](nextdeck-test-events/README.md) helper crate.

## Project selection

Open a workspace other than the current directory with `--working-dir`:

```sh
nextdeck --working-dir ../my-workspace
```

The directory may be the workspace root or any directory inside it. Nextdeck
resolves the workspace root before starting Nextest or xtasks.

Run `nextdeck --help` for the full command-line reference.

## License

Nextdeck is licensed under the [Apache License 2.0](LICENSE).
