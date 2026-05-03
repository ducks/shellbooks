# shellbooks

A terminal audiobook player and library manager written in Rust, using
[ratatui](https://github.com/ratatui-org/ratatui) and
[crossterm](https://github.com/crossterm-rs/crossterm).

Inspired by [shelltrax](https://github.com/ducks/shelltrax) (music) and
[shellcast](https://github.com/ducks/shellcast) (podcasts) — the same cmus-style
keyboard-driven interface, adapted for audiobooks.

## Features

- Two-pane Library view: books on the left, chapters on the right, Tab to focus
- File browser for explicit imports — pick a directory of audio files or a
  single `.m4b`, press `a` to add it to the library
- Per-book progress: position resumes to the second, autosaves every 10s
- Chapter detection: parses Nero-style chapter atoms in `.m4b` files,
  synthesizes one chapter per file for multi-file books with cleaned
  filename titles
- Variable speed (0.5x – 3.0x) — note: pitch shifts with tempo today;
  pitch-preserving via tdpsola is on the roadmap
- Tag extraction (id3 for mp3, mp4ameta for m4b/m4a): title, author,
  narrator, series, year, embedded cover art
- Cover art resolution: sidecar `cover.jpg` / `cover.png` first, then
  embedded artwork written to `~/.cache/shellbooks/covers/`
- Persistent footer with playback state on every screen
- Info modal (`i`) showing the selected book's full metadata
- Library + progress persisted to `~/.local/share/shellbooks/library.json`

## Status

Daily-usable. The big remaining gaps are bookmarks (model exists, UI
doesn't), pitch-preserving speed, theme support, and a sleep timer.

## Installation

```bash
git clone git@github.com:ducks/shellbooks.git
cd shellbooks
cargo build --release
./target/release/shellbooks
```

A `shell.nix` is provided for NixOS users.

### System dependencies

Linux needs ALSA development headers for `rodio` to build:

```bash
sudo apt install libasound2-dev   # Debian / Ubuntu
sudo dnf install alsa-lib-devel    # Fedora
```

NixOS users get this from `shell.nix` automatically.

## Configuration

Copy `config.example.toml` to `~/.config/shellbooks/config.toml` if you
want to override defaults — none of it is required for the app to work.

```toml
library_paths = ["~/Audiobooks"]
default_speed = 1.0
sleep_timer_minutes = 0
theme = "gruvbox"             # not yet wired
finish_threshold_seconds = 30
```

`library_paths` is currently advisory — books still need to be imported
explicitly through the browser. A future "rescan configured roots" action
will use it.

## Usage

1. Launch shellbooks
2. Press `3` to open the file browser
3. Navigate with `j`/`k` and `enter` (or `l`) to descend
4. Highlight an audiobook directory or `.m4b` file, press `a` to import
5. Press `1` to return to the library, `enter` to start playing

## Keybindings

### Global

| Key             | Action                                     |
|-----------------|--------------------------------------------|
| `q`             | Quit                                       |
| `1`             | Library                                    |
| `2`             | Bookmarks                                  |
| `3`             | Browser                                    |
| `i`             | Toggle info modal                          |
| `space` / `c`   | Play / pause                               |
| `+` / `=` / `-` | Speed up / down (5% steps)                 |
| `,` / `.`       | Seek ±10s                                  |
| `[` / `]`       | Seek ±60s                                  |
| `b`             | Next chapter                               |
| `z`             | Previous chapter                           |

### Library

| Key            | Action                                      |
|----------------|---------------------------------------------|
| `tab`          | Toggle focus between Books and Chapters     |
| `j` / `k`      | Move within the focused pane                |
| `enter`        | Books pane: resume saved progress           |
|                | Chapters pane: play from this chapter       |
| `a`            | Open the browser to add a book              |
| `d`            | Delete highlighted book from the library    |

### Browser

| Key            | Action                                      |
|----------------|---------------------------------------------|
| `j` / `k`      | Move down / up                              |
| `g` / `G`      | Top / bottom                                |
| `enter` / `l`  | Descend into directory                      |
| `h` / `←` / `backspace` | Up one directory                   |
| `a`            | Import the highlighted path                 |
| `esc`          | Back to Library                             |

## Library Conventions

A book is one of:

- A **directory of audio files** (mp3, m4a, m4b, flac, ogg, opus, wav).
  One chapter per file. Filenames like `01 - Prologue.mp3` get cleaned
  to `Prologue`.
- A **single `.m4b`** with internal chapter markers. The chpl atom is
  parsed; if absent, the whole file is one chapter.

Tags are read from the first audio file. For multi-file books, the
album tag is treated as the book title (per-track `title` is the
chapter, not the book) — fixes the "Track 001" display bug.

Cover art comes from a sidecar `cover.jpg` / `cover.png` /
`folder.jpg` / `folder.png` next to the audio, falling back to embedded
artwork written into `~/.cache/shellbooks/covers/<book_id>.<ext>`.

## Roadmap

Loose order of likely next work:

- Bookmarks UI: `m` to mark, `2` to list, `enter` to jump
- Pitch-preserving variable speed (tdpsola adapter as a streaming
  rodio source — real DSP project)
- Theme parsing (TOML themes like shelltrax)
- Sleep timer countdown
- Cover art rendering in-terminal (kitty / sixel / iterm2 via
  `ratatui-image`)
- Network sources (Audiobookshelf API integration)

## License

MIT OR Apache-2.0
