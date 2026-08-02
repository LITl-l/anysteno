# anysteno — Design

## Goals

Cross-platform (Linux/Windows/macOS) stenography for **any keyboard** and **any
language**, delivered as a fast, tiny, single binary that is also a gentle
trainer for beginners. English and Japanese ship officially; more languages are
data, not code.

## Layering

The overriding principle is to isolate everything hard-to-test (OS input,
injection, GUI) from the logic, so the logic can be exhaustively unit-tested and
is identical on every OS.

```
┌──────────────────────────────────────────────────────────┐
│ steno-app (egui)   screens · keyboard view · routing      │
├──────────────────────────────────────────────────────────┤
│ platform           capture (rdev) · inject (enigo)        │  thin OS shims
├──────────────────────────────────────────────────────────┤
│ steno-core         pure engine, NO I/O, NO OS deps        │  fully tested
│   pack → layout → chord → stroke → dictionary → engine     │
│   + reverse index · curriculum/drill · session stats       │
└──────────────────────────────────────────────────────────┘
```

Only `capture` and `inject` touch the OS keyboard; only `app` touches the
screen. `steno-core` depends on nothing but `serde`.

## Core modules

- **`stroke`** — `StenoKey` (id, letter, bank) and `render_stroke`, which turns a
  set of pressed key ids into a canonical stroke string, applying the steno
  hyphen rule (a `-` separates the hands when a chord has no middle/vowel key).
  `parse_stroke` is its inverse, recovering the key ids from a stroke string.
  That direction is what lets the UI show *which keys to press* for a word, and
  what lets the shipped packs be checked for strokes that can never be chorded.
- **`layout`** — physical key name → steno key id, plus the ordered key list.
  Physical keys use platform-neutral names (`"KeyS"`, `"SemiColon"`), so the core
  never sees an OS type.
- **`chord`** — `ChordAccumulator`. Presses grow the current chord; the chord is
  emitted when the **last** held key is released. This tolerates staggered
  presses, which is what makes ordinary keyboards usable.
- **`dictionary`** — `Dictionary` (stroke→text with a derived prefix index) and
  `Translator`, which buffers strokes and resolves them with **greedy longest
  match**: it waits while the buffer could still be the prefix of a longer
  multi-stroke entry, otherwise commits the best match. Unmatched strokes become
  an explicit `NoMatch` (never silently dropped). `flush()` force-resolves
  pending strokes (on space/idle).
- **`pack`** — loads a `Pack` (layout + dictionary + meta) from TOML/JSON, with an
  optional `user.json` overlay that wins over shipped entries. Errors are typed
  (`PackError`) so the app keeps the previous pack and shows why.
- **`engine`** — the single façade the app uses. Owns the pack, accumulator and
  translator; converts raw `KeyEvent`s into `EngineOutput` (stroke + translations
  + undo flag). Handles enable/disable and the pack's undo stroke.
- **`reverse`** — `ReverseIndex`, the dictionary read backwards: text → the
  strokes that write it. A dictionary may spell the same text several ways, so
  it also picks a *best* one (fewest strokes, then fewest characters, then
  alphabetical — the last tiebreak makes the choice independent of hash order).
  Backs the dictionary browser and every stroke hint.
- **`lesson`** — `Curriculum::derive` generates a progression from a pack alone,
  and `Drill` runs one. See [Generated curricula](#generated-curricula).
- **`stats`** — `SessionStats`: WPM and accuracy. Clock-free; the caller supplies
  elapsed milliseconds, which keeps `std::time` out of the core and lets tests
  assert exact rates instead of sleeping.
- **`defaults`** — the official packs embedded via `include_str!`, so the app
  works with zero setup.

## Data flow

```
key events ─▶ engine.on_key ─▶ EngineOutput ─▶ [StenoEvent] ─▶ active screen
                                                               ├─ Learn:    judge against the drill target
                                                               └─ Practice: append, and inject if enabled
keyboard view (held keys / hint chord) redraws every event
```

Engine output is normalised into a list of `StenoEvent`s (`Stroke`, `Text`,
`NoMatch`, `Undo`) before any screen sees it. Screens therefore never touch the
engine, the capture thread or the injector — they consume a list of things that
happened, which is what keeps them small enough to be obviously correct without
a UI test harness.

## Generated curricula

Lessons are **generated, not authored**. Requiring a hand-written lesson plan per
language would break the project's central promise that a language is just a
folder, so the progression is derived from the two files that already exist:

1. Walk `layout.order()` — already grouped by hand — and introduce keys a few at
   a time. The middle bank goes second, straight after the first group of
   left-hand keys: it is by definition the bank that joins the hands, which in
   practice means the vowels, and almost nothing is reachable without one.
   Teaching it early is what makes lesson two produce words instead of letters.
2. A lesson drills the dictionary entries reachable with the keys taught so far
   that also use at least one key just introduced.
3. Chunks that reach too few entries carry their keys into the next lesson —
   Japanese has no consonant-only entries at all, so its first chunk would
   otherwise be empty.
4. Entries that overflow a lesson's item cap become **review lessons** at the
   end. Without this they would be lost: every key is taught by the last lesson,
   so no later lesson would ever pick them up.

Candidates are deduplicated by output *text*, not by stroke. `en-beginner`
spells "r" both as `R` and `-R`, and a drill can only check the text that came
out, not which key produced it — offering both would set a target that cannot be
marked wrong.

## Key decisions & trade-offs

- **Chording, not sequential typing.** Required for "any keyboard" and real
  steno feel. Cost: needs press/release events and some keyboards limit
  simultaneous keys; short chords are fine on virtually all keyboards.
- **Beginner theory = standard Plover geometry + a tiny curated dictionary.**
  Recognisable mechanics, but small and intuitive. The default English dict is
  deliberately **single-stroke only** so common words fire instantly (a word that
  is also the prefix of a multi-stroke entry must wait one stroke — avoided by
  design). Multi-stroke support still exists and is unit-tested.
- **Two input paths, one engine.** In-app reads egui's key events (no OS
  permission, no double-typing because no text widget is focused); system-wide
  reads a global `rdev` capture thread and injects via `enigo`.
- **System-wide does not suppress keys yet.** `rdev::listen` observes but cannot
  consume events portably; true suppression needs per-OS grab APIs. Shipped as a
  known limitation rather than faking it.
- **Extensibility via files, not plugins.** A language is a folder; adding one
  never recompiles the binary. This satisfies "extend itself" and "user
  customizable dictionary" with one mechanism.
- **The keyboard picture lives in the app, not the core.** `steno-core` must not
  learn what a keyboard looks like. The geometry table is keyed by the same
  platform-neutral names `layout.toml` uses, so one table renders every pack.
- **System-wide output always leads with a space.** Whether a word is the first
  one is answerable for anysteno's own buffer and unanswerable for someone
  else's document, so injection follows the usual steno space-before convention
  rather than guessing from local state and running words together.
- **Injection is confined to the Practice screen.** A drill's answers appearing
  in whatever window happens to be behind anysteno would be a nasty surprise.

## Testing

`steno-core` has 89 unit tests covering hyphenation, stroke parsing and its
round-trip against rendering, chord accumulation (including
staggered/duplicate/reset), greedy multi-stroke translation, pack
parsing/errors/overlays, reverse-index ranking and tie-breaking, curriculum
generation against both shipped packs, drill state, and statistics. The
`steno-app` layer adds 4 covering settings migration and progress tracking. They
need no display and run in milliseconds.

Two invariants are asserted against the shipped packs specifically:

- **every stroke is chordable** — each dictionary key must parse in its layout
  *and* render back to itself. This caught four dead entries (`HELP`, `PEST`,
  `REST`, `TEST`): `-S` sorts after `-T` in the right bank, so chording "PEST"
  produces the stroke `PETS`, which was never in the dictionary. Those words
  could not be typed at all.
- **every word is taught somewhere** — no dictionary entry may be dropped by
  curriculum generation.

The GUI layer is thin and validated by launching the app and driving it.

## Build

A Nix `shell.nix`/`flake.nix` provides the Rust toolchain and the Linux system
libraries (X11/Wayland/GL for egui, XTest for rdev/enigo). Release profile is
tuned for size (`opt-level="z"`, LTO, `strip`, `panic="abort"`): ~5.0 MB single
binary.
