# Contributing to Kiri

Thanks for helping. Issues and pull requests are welcome.

## Before you start

For anything larger than a small fix, open an issue first so we can agree on
the approach. Kiri aims to be small and fast: changes that add a dependency, a
background process or startup time need a good reason.

## Building and testing

```sh
cargo xtask host      # target/release/kiri, signed with the virtualization entitlement
cargo test            # unit tests
tests/e2e.sh          # the Docker compatibility suite, against a running `kiri daemon`
```

Use `cargo xtask host`, not plain `cargo build`, for a binary you run as the
daemon: without the virtualization entitlement the VM will not start. The
guest needs `cargo xtask kernel`, `cargo xtask disk` and `cargo xtask guest`
once; see the README.

`tests/e2e.sh` only removes containers, networks and volumes it created.

## Every feature ships on macOS, Windows and Linux together

- A feature comes in one pull request, for all three systems, built and
  tested on each of them before it's merged.
- No `if (WIN)` or Kiri Dev (`flavor::dev()`) gate that hides a feature from
  a system or from Kiri. Words may differ by system ("Show in Finder"); what
  the window offers may not. Plumbing (contexts, update URLs) may differ.
- `python3 scripts/ui-test.py` must pass. Its parity test opens the window as
  each system and fails when one shows something another doesn't, unless
  `tests/ui/parity-allow.txt` gives the reason. A new line there needs a
  maintainer's OK.

## Pull requests

- Keep each pull request to one change, with a message that says what it does
  and why.
- Run `cargo fmt`, `cargo clippy` and the tests. After changing Rust
  dependencies or bundled tool versions, run `scripts/ci/notices.sh` and commit
  the updated NOTICE.
- Sign your commits and the [Contributor License Agreement](CLA.md) when CLA
  Assistant asks for it on your first pull request.

By submitting a contribution you agree it is licensed under Apache-2.0, as
described in the README and the Contributor License Agreement.
