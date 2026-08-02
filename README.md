# anysteno

**Stenography for any keyboard, any language.** A fast, tiny, single-binary
steno typing app for Linux, Windows and macOS — no special hardware, no steno
machine, just the keyboard you already have.

anysteno turns simultaneous key presses ("chords") into words, the way a
stenotype does. It is built to *teach* that skill: it draws your actual
keyboard, lights up the keys as you hold them, and drills you through a lesson
plan it generates from whichever language you're learning. English and Japanese
ship by default; any language can be added by dropping in a folder of plain
files.

> Status: the steno engine is complete and fully tested, and the app is a
> working trainer. System-wide injection works but does not yet *suppress* the
> raw keystrokes — see [Limitations](#limitations).

---

## Features

- **Any keyboard.** Standard QWERTY chording (Plover-style). No n-key-rollover
  keyboard required for short chords.
- **A live keyboard.** The main view is your real keyboard with the steno
  letters drawn on it, colour-coded by hand, lighting up as you press. It is
  generated from the active pack, so it is correct for any language.
- **Lessons that generate themselves.** anysteno derives a progression from a
  pack's layout and dictionary: introduce a few keys, then drill the words those
  keys can now reach. Adding a language gets you a curriculum for free — you
  never write a lesson plan.
- **Words per minute and accuracy**, per drill and while practising.
- **Reverse lookup.** Search the dictionary by word *or* by stroke and see the
  chord painted onto the keyboard. "How do I write this?" has an answer.
- **Any language.** A language is a folder of files (`layout.toml` +
  `dict.json`). Add one without recompiling. Ships with `en-beginner` and
  `ja-beginner`.
- **Two output destinations.** Type into anysteno's own page, or inject into
  whatever application is focused.
- **Customisable dictionaries.** Drop a `user.json` next to any pack to override
  or add entries; your edits never touch the shipped files.
- **Fast & tiny.** Pure-Rust core, single self-contained binary, no runtime.

## The app

Four screens, in the sidebar:

| Screen | What it's for |
|---|---|
| **Learn** | The generated lesson plan, and the drill that runs it. Shows the target word, the chord that writes it, and your score. |
| **Practice** | A free page to write on, with a live feed of stroke → result. This is the only screen that can type into other applications. |
| **Dictionary** | Search by word or by stroke; the result's chord is drawn on the keyboard. |
| **Settings** | Language, hints, theme, and anything that failed to load. |

The drill is deliberately forgiving: a wrong answer never advances and never
ends the run, it just reveals the chord on the keyboard. Recalling a chord is
the hard part of steno, and hiding the answer after a miss only stalls you.

## How steno works here

Real stenography presses several keys **at once**; the whole group is one
*stroke*, looked up in a dictionary to produce a word. anysteno detects a stroke
when the **last** held key is released, so slightly-staggered presses still
count as one chord.

A stroke is written in *steno order* using the pack's key letters, e.g. holding
the keys for `K`, `A`, `T` makes the stroke **`KAT`** → `cat`. Chords with no
vowel get a hyphen to separate the two hands (e.g. `S` + `-T` → `S-T`).

Steno order also means a stroke is **not** spelled the way the English word is.
The right-hand bank runs `F R P B L G T S D Z`, so the keys for "s" and "t"
always come out as `-TS`, never `-ST`. A dictionary entry whose letters run
against that order can never be typed, no matter which keys you hold — the
shipped packs are checked against this at build time.

### Default English layout (beginner)

Standard Plover hand position on QWERTY:

```
   S T P H  *       -F -P -L -T -D
   S K W R  *       -R -B -G -S -Z
        A O   E U
```

The `en-beginner` pack ships a small curated word list (`cat`, `the`, `world`,
`stop`, …) plus **fingerspelling** for the whole alphabet, so you can always
spell a letter. Letters with their own steno key are a single press (`S` → `s`);
the rest use short Plover-style chords (`KR` → `c`, `PH` → `m`, `TPH` → `n`).
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
| Linux (Wayland) | global capture is limited by the compositor; typing into anysteno always works |
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

Copy `en-beginner` as a starting point, edit the files, then press **Reload
packs** in Settings. Anything that fails to parse is reported there rather than
silently ignored. To just add words, create `user.json` in an existing pack:

```json
{ "KAT": "kitty", "TPHU": "new word" }
```

Your new words are picked up by the lesson generator, the dictionary search and
the keyboard view automatically — there is nothing else to update.

## Limitations (this version)

- **System-wide keys are not suppressed yet.** When typing into other apps the
  raw letters still reach them *in addition* to the injected translation. True
  suppression needs per-OS grab APIs (Windows/macOS hooks, Linux uinput); it is
  planned. Typing into anysteno is unaffected and fully usable.
- **Japanese needs a CJK font.** anysteno auto-loads Noto Sans CJK / system CJK
  fonts if present; otherwise kana render as boxes. Install Noto Sans CJK.
- **Wayland** global capture depends on the compositor.

## Architecture

Three layers; the brain has no OS dependencies and is fully unit-tested.

```
app (egui)        screens, input routing, output routing
platform          rdev (capture)  ·  enigo (inject)  — thin OS shims
steno-core        chord → stroke → dictionary → engine   (pure, tested)
                  + curriculum · reverse index · stats
```

Lessons, reverse lookup and scoring all live in the pure core, so they are
decided without a screen and covered by unit tests. See [`DESIGN.md`](DESIGN.md)
for the full design.

## Development

```sh
nix-shell --run 'cargo test'      # 93 unit tests
nix-shell --run 'cargo clippy --all-targets'
```

## License

MIT © fairenemo
