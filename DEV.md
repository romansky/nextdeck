## Local Publishing

Project automation lives in `xtask` and is available through the Cargo alias:

```sh
cargo xtask --help
```

Useful local publishing commands:

- `cargo xtask check --allow-dirty`: run format check, tests, and package verification.
- `cargo xtask package --allow-dirty`: create and verify `target/package-verify/package/nextdeck-*.crate`.
- `cargo xtask publish-local --allow-dirty`: package, install from the verified package directory, and verify `PATH` resolves to the installed binary.
- `cargo xtask install-path`: install directly from the workspace checkout.
- `cargo xtask release --allow-dirty --skip-sign`: build a local release archive in `target/dist`.
- `cargo xtask homebrew-formula --github-repo owner/nextdeck --dist-dir target/dist --output Formula/nextdeck.rb`: render a Homebrew formula from release checksums.
- `cargo xtask nextdeck-info --format json`: expose this repo's xtasks to nextdeck.

See `docs/xtask-integration/README.md` for the JSON contract other repos can expose.
