# shellbooks

A terminal audiobook player and library manager. Inspired by
[shelltrax](https://github.com/ducks/shelltrax) and
[shellcast](https://github.com/ducks/shellcast).

## Features

- Browse a local audiobook library across one or more root directories
- Books = directories of chapter files (mp3) or single `.m4b` files
- Chapter navigation, position resume per book, finished/unfinished tracking
- Series grouping
- Cover art (sidecar `cover.jpg`/`cover.png` or embedded tags)
- Pitch-preserving variable speed via `tdpsola` (0.75x – 3.0x)
- Sleep timer
- Bookmarks with optional notes

## Status

Early scaffold. Not usable yet.

## Configuration

Copy `config.example.toml` to `~/.config/shellbooks/config.toml` and point
`library_paths` at your audiobook directories.

## Build

```
cargo build --release
```

A `shell.nix` is provided for NixOS users.

## License

MIT OR Apache-2.0
