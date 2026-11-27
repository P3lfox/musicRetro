# Reproductor Retro

Lightweight native desktop music player with a 2000s console-menu aesthetic.
Built with Rust + Slint. No browser, no Electron, no webviews.

- **Targets:** Windows (primary), Linux; Android/iOS planned.
- **Spec:** see [`docs/SPEC.md`](docs/SPEC.md).

## Development

```sh
cargo run -p app-desktop            # debug
cargo build --release -p app-desktop
cargo fmt --all && cargo clippy --all-targets -- -D warnings

# M1: command-line player (p pause, n/b next/prev, s <secs> seek, +/- volume, r repeat, i info, q quit)
cargo run --release -p retro-core --example play -- <file-or-folder>...
```

## Downloads

Windows builds are published on the GitHub **Releases** page.
# musicRetro
