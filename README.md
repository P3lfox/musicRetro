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
```

## Downloads

Windows builds are published on the GitHub **Releases** page.
# musicRetro
