# Xtask Integration

Nextdeck can help expose project automation commands built with [`xtask`](https://github.com/matklad/cargo-xtask)
convention in the TUI accessible via "X" global command.

If you are using Clap CLI its recommended to use the `nextdeck-helper` crate for fast and simple integration,
alternatively you can use the provided schema and manually generate the manifest to drive the integration.

Nextdeck integration happens by reading the output produced by running:

```sh
cargo xtask nextdeck-info --format json
```

The command should print the generated manifest and exit successfully.

## Prerequisites

`cargo xtask` must work from the Cargo workspace root. Most projects provide
it with an alias:

```toml
# .cargo/config.toml
[alias]
xtask = "run --package xtask --"
```

## Integration Via `nextdeck-helper` Crate

### Clap Setup

Add the helper to the xtask crate:

```toml
[dependencies]
nextdeck-helper = { version = "0.1", features = ["xtask-clap"] }
```

Call `xtask_clap_info!` before parsing the command line:

```rust
use clap::Parser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    nextdeck_helper::xtask_clap_info!(Cli);

    let cli = Cli::parse();
    // Dispatch the existing command.
    Ok(())
}
```

The helper reads visible top-level subcommands and their visible, named arguments. Command descriptions, argument help,
defaults, and possible values come from the Clap definitions.

Supported argument shapes are:

| Clap shape                           | Nextdeck value |
|--------------------------------------|----------------|
| `SetTrue` or `SetFalse` flag         | Boolean        |
| Single value with possible values    | Enum           |
| Single value with an integer default | Number         |
| Other single value                   | Text           |

Give each integrated argument a long name such as `--allow-dirty`. Positional, repeated, variadic, count, hidden, and
nested-subcommand arguments are not exposed in Nextdeck; they continue to work when the xtask is run directly.

## Manual Integration

Projects that do not use Clap can implement the same endpoint directly. It must write only JSON to stdout; diagnostics
may be written to stderr.

Write JSON string based on XtaskManifestV1 from the TypeScript scehma below to stdout:

```ts
type XtaskManifestV1 = {
    schema_version: 1;
    commands: XtaskCommand[];
};

type XtaskCommand = {
    name: string; // ASCII letters, digits, "_", and "-"
    about?: string;
    args?: XtaskArgument[]; // Defaults to an empty list
};

type XtaskArgument = {
    name: string; // ASCII letters, digits, "_", and "-"
    long?: string; // Preferred flag name, without "--"
    short?: string; // Single character; descriptive metadata only
    help?: string;
    required?: boolean; // Defaults to false
    value: XtaskValue;
};

type XtaskValue =
    | { type: "bool"; default?: boolean } // Defaults to false
    | { type: "string"; default?: string }
    | { type: "number"; default?: number } // Signed 64-bit integer
    | {
    type: "enum";
    values: string[]; // Must contain at least one value
    default?: string; // Must be present in values
};
```

Nextdeck invokes an argument by its `long` name, falling back to `name`. A boolean that differs from its default is
passed as a flag. Other values are passed as `--name value`, except optional values at their declared defaults, which
are omitted. Argument values are remembered per Cargo workspace.

## Source References

- [`xtask-clap` helper and manifest types](https://docs.rs/crate/nextdeck-helper/latest/source/src/xtask.rs)
- [Nextdeck's xtask integration](../../xtask/src/main.rs)
