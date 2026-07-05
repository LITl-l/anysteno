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
│ steno-app (egui)   GUI · toggles · live view · routing    │
├──────────────────────────────────────────────────────────┤
│ platform           capture (rdev) · inject (enigo)        │  thin OS shims
├──────────────────────────────────────────────────────────┤
│ steno-core         pure engine, NO I/O, NO OS deps        │  fully tested
│   pack → layout → chord → stroke → dictionary → engine     │
└──────────────────────────────────────────────────────────┘
```

Only `capture` and `inject` touch the OS keyboard; only `app` touches the
screen. `steno-core` depends on nothing but `serde`.

## Core modules

- **`stroke`** — `StenoKey` (id, letter, bank) and `render_stroke`, which turns a
  set of pressed key ids into a canonical stroke string, applying the steno
  hyphen rule (a `-` separates the hands when a chord has no middle/vowel key).
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
- **`defaults`** — the official packs embedded via `include_str!`, so the app
  works with zero setup.

## Data flow

```
key events ─▶ engine.on_key ─▶ EngineOutput
                                 ├─ In-app:     append to display buffer
                                 └─ System-wide: enigo injects into focused app
live view (held keys / last stroke / pending) updates every event
```

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

## Testing

`steno-core` has 35 unit tests covering hyphenation, chord accumulation
(including staggered/duplicate/reset), greedy multi-stroke translation, pack
parsing/errors/overlays, and the shipped packs. They need no display and run in
milliseconds. The GUI/platform layer is thin and validated by launch + manual
smoke test per OS.

## Build

A Nix `shell.nix`/`flake.nix` provides the Rust toolchain and the Linux system
libraries (X11/Wayland/GL for egui, XTest for rdev/enigo). Release profile is
tuned for size (`opt-level="z"`, LTO, `strip`, `panic="abort"`): ~4.7 MB single
binary.
