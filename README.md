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
> working trainer. Typing into other applications holds back the raw keystrokes,
> so only the translation lands — see [Typing into other applications](#typing-into-other-applications).

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
  `ja-beginner` — and Japanese renders on a machine with no fonts installed,
  because the kana come built in.
- **Two output destinations.** Type into anysteno's own page, or into whatever
  application is focused. There the raw letters are held back, so the other app
  sees the translation and nothing else — where the OS grants the access to do
  that, and anysteno says plainly when it doesn't.
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

On Linux the key-suppressing capture backend needs the `libevdev` headers at
build time (`libevdev-dev` / `libevdev-devel`; the Nix shell supplies it). To
build without it, use `cargo build --no-default-features` — capture then
observes keys rather than consuming them.

### Typing into other applications

Learning inside anysteno needs no permissions at all. Typing into *other* apps
means taking keys from the rest of the system, which every OS gates:

| OS | What it needs |
|----|---------------|
| Linux | Read/write on `/dev/input` and `/dev/uinput` — usually `sudo usermod -aG input $USER`, then log back in. Works on X11 and Wayland alike, because it reads below the display server. |
| macOS | Grant anysteno **Accessibility** permission in System Settings. |
| Windows | Nothing; works as-is. |

If anysteno cannot get that access it does not fail silently or pretend: it
falls back to watching keys instead of consuming them, and the Practice screen
tells you exactly which permission is missing.

Keys anysteno does not chord are *never* taken. Escape, modifiers, Tab, function
keys and any unbound letter always reach the system, so a capture can't leave
you unable to switch away.

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

## Where the edges are

Nothing here is a "todo" — these are the boundaries of what the surrounding
systems allow, and anysteno reports each one in place rather than failing
quietly.

- **Kanji outside the shipped pack.** anysteno embeds a kana subset of Noto Sans
  CJK (~36 KB), so the Japanese pack renders on a machine with no fonts
  installed. A pack *you* write using kanji needs a system CJK font, because
  bundling all ~20,000 of them would cost more than the rest of the binary
  several times over. Settings says so when no system CJK font is present.
- **GNOME on Wayland can only be typed into via XWayland.** Injection uses the
  compositor's `virtual-keyboard-v1` protocol, which wlroots compositors (Sway,
  Hyprland, river) and KDE implement and GNOME does not. The remaining route for
  GNOME is the desktop portal, and anysteno does not take it: enigo 0.2.1's
  portal backend `unwrap()`s a D-Bus error when no portal is running
  (`linux/libei.rs`), which aborts the process — and because anysteno builds
  with `panic = "abort"` for size, that is not even catchable. Enabling it was
  tried and reverted. Under GNOME/Wayland, X11 applications (via XWayland) still
  receive injected text; native Wayland ones do not. A GNOME session on X11 is
  unaffected, as are all other compositors.
- **Keyboards that can't report the chord.** Some membrane keyboards physically
  cannot report certain 3+ key combinations (they lack n-key rollover). No
  software can see a keypress the hardware never sends. Short chords are fine on
  virtually all keyboards, which is what the beginner packs are built from.

## Architecture

Three layers; the brain has no OS dependencies and is fully unit-tested.

```
app (egui)        screens, input routing, output routing
platform          rdev grab/listen (capture)  ·  enigo (inject)  — thin OS shims
steno-core        chord → stroke → dictionary → engine   (pure, tested)
                  + curriculum · reverse index · stats
```

Lessons, reverse lookup and scoring all live in the pure core, so they are
decided without a screen and covered by unit tests. See [`DESIGN.md`](DESIGN.md)
for the full design.

## Development

```sh
nix-shell --run 'cargo test'      # 102 unit tests
nix-shell --run 'cargo clippy --all-targets'

# The build without the key-suppressing backend is supported too, so it is
# worth linting as well.
nix-shell --run 'cargo clippy -p steno-app --no-default-features --all-targets'
```

## License

MIT © fairenemo
