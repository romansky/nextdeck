## Local Publishing

Project automation lives in `xtask` and is available through the Cargo alias:

```sh
cargo xtask --help
```

Useful local publishing commands:

- `cargo xtask tui-check --allow-dirty`: run checks for the TUI package.
- `cargo xtask tui-publish-local`: install the TUI directly from the workspace checkout.
- `cargo xtask helper-check --allow-dirty`: run checks for `nextdeck-helper`.
- `cargo xtask helper-package --allow-dirty`: create a verified `nextdeck-helper` package.
- `cargo xtask helper-publish-local --allow-dirty`: install and smoke-test `nextdeck-helper` from its package.
- `cargo xtask tui-release --allow-dirty --skip-sign`: build a local release archive in `target/dist`.
- `cargo xtask tui-homebrew-formula --github-repo owner/nextdeck --dist-dir target/dist --output Formula/nextdeck.rb`: render a Homebrew formula from release checksums.
- `cargo xtask nextdeck-info --format json`: expose this repo's xtasks to nextdeck.

See `docs/xtask-integration/README.md` for the manifest format other repos can expose.
