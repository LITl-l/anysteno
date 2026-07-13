# anysteno

**Stenography for any keyboard, any language.** A fast, tiny, single-binary
steno typing app for Linux, Windows and macOS — no special hardware, no steno
machine, just the keyboard you already have.

anysteno turns simultaneous key presses ("chords") into words, the way a
stenotype does, and shows you exactly what you typed as steno — so it doubles as
a friendly trainer for beginners. English and Japanese ship by default; any
language can be added by dropping in a folder of plain files.

> Status: first working version. The steno engine is complete and fully tested;
> the GUI works with in-app practice on all three platforms. System-wide
> injection works but does not yet *suppress* the raw keystrokes — see
> [Limitations](#limitations).

---

## Features

- **Any keyboard.** Standard QWERTY chording (Plover-style). No n-key-rollover
  keyboard required for short chords.
- **Any language.** A language is a folder of files (`layout.toml` + `dict.json`).
  Add one without recompiling. Ships with `en-beginner` and `ja-beginner`.
- **Two output modes, toggleable at runtime.**
  - *In-app* — chord inside anysteno's window and watch the translation. Great
    for learning; works everywhere with no permissions.
  - *System-wide* — inject translated text into whatever app is focused.
- **Beginner-friendly.** A live view shows the keys you're holding, the stroke
  you made, and the word it produced. Unknown chords are shown, never swallowed.
- **Customisable dictionaries.** Drop a `user.json` next to any pack to override
  or add entries; your edits never touch the shipped files.
- **Fast & tiny.** Pure-Rust core, single self-contained binary, no runtime.

## How steno works here

Real stenography presses several keys **at once**; the whole group is one
*stroke*, looked up in a dictionary to produce a word. anysteno detects a stroke
when the **last** held key is released, so slightly-staggered presses still
count as one chord.

A stroke is written in *steno order* using the pack's key letters, e.g. holding
the keys for `K`, `A`, `T` makes the stroke **`KAT`** → `cat`. Chords with no
vowel get a hyphen to separate the two hands (e.g. `S` + `-T` → `S-T`).

### Default English layout (beginner)

Standard Plover hand position on QWERTY:

```
   S T P H  *       -F -P -L -T -D
   S K W R  *       -R -B -G -S -Z
        A O   E U
```

The `en-beginner` pack ships a small curated word list (`cat`, `the`, `world`,
`stop`, …) plus single-key **fingerspelling** so you can always spell a letter.
It is intentionally small and intuitive — extend it via `user.json` or swap in a
full Plover dictionary by writing a pack.

### Default Japanese layout (beginner)

Same finger positions, with a fifth vowel `I`, so consonant+vowel chords spell
kana: `KA` → か, `SI` → し, `TPHA` → な, `-B` → ん.

## Install & run

anysteno builds with a standard Rust toolchain. On NixOS the provided shell
supplies the toolchain and the needed system libraries:

```sh
nix-shell            # or: nix develop   (flake)
cargo run -p steno-app --release
```

On other systems: install Rust, then `cargo run -p steno-app --release`. The
release binary is a single file at `target/release/anysteno`.

### Platform notes

| OS | Global capture / injection needs |
|----|----------------------------------|
| Linux (X11) | works out of the box |
| Linux (Wayland) | global capture is limited by the compositor; in-app mode always works |
| macOS | grant **Accessibility** permission to capture and inject |
| Windows | works; no special permission |

## Adding or customising a language

Packs live in your config dir (created on first run):

- Linux: `~/.config/anysteno/packs/`
- macOS: `~/Library/Application Support/anysteno/packs/`
- Windows: `%APPDATA%\anysteno\anysteno\packs\`

Each pack is a folder:

```
my-lang/
  pack.toml     # code, name, lang, undo_stroke
  layout.toml   # steno key order + physical->steno key map
  dict.json     # { "STROKE": "text", ... }
  user.json     # optional overrides (wins over dict.json)
```

Copy `en-beginner` as a starting point, edit the files, restart, and pick your
language from the dropdown. To just add words, create `user.json` in an existing
pack:

```json
{ "KAT": "kitty", "TPHU": "new word" }
```

## Limitations (this version)

- **System-wide keys are not suppressed yet.** In system-wide mode the raw
  letters still reach the focused app *in addition* to the injected translation.
  True suppression needs per-OS grab APIs (Windows/macOS hooks, Linux uinput);
  it is planned. In-app mode is unaffected and fully usable.
- **Beginner English omits some letters** (c, i, j, m, n, q, v, x, y have no
  single-key fingerspelling in the tiny default). Add them in `user.json`.
- **Japanese needs a CJK font.** anysteno auto-loads Noto Sans CJK / system CJK
  fonts if present; otherwise kana render as boxes. Install Noto Sans CJK.
- **Wayland** global capture depends on the compositor.

## Architecture

Three layers; the brain has no OS dependencies and is fully unit-tested.

```
app (egui)        GUI, toggles, live view, output routing
platform          rdev (capture)  ·  enigo (inject)  — thin OS shims
steno-core        chord → stroke → dictionary → engine   (pure, tested)
```

See [`DESIGN.md`](DESIGN.md) for the full design.

## Development

```sh
nix-shell --run 'cargo test'      # 35 core unit tests
nix-shell --run 'cargo clippy --all-targets'
```

## License

MIT © fairenemo
