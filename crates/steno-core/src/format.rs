//! Plover-style formatting of dictionary translations.
//!
//! [`Translator`](crate::Translator) yields raw dictionary strings such as
//! `{^ing}` or `{.}`; this module interprets that translation language
//! (attach, glue, case, punctuation, retro spacing) and renders the text a
//! user should see. Each step also reports the minimal keystroke delta
//! ([`Output`]) needed to bring already-typed text up to date, so a caller can
//! drive a keyboard without re-typing everything.
//!
//! Pure and clock-free like the rest of the core: no I/O, no key sending.

/// The edit that turns the previously rendered text into the new one when
/// typed at its end: delete `backspaces` chars, then type `text`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Output {
    /// Characters (`char`s, not bytes) to delete from the end first.
    pub backspaces: usize,
    /// Text to type after the backspaces.
    pub text: String,
}

impl Output {
    /// Minimal delta from `before` to `after`, counted in `char`s.
    fn diff(before: &str, after: &str) -> Self {
        let common = before
            .chars()
            .zip(after.chars())
            .take_while(|(b, a)| b == a)
            .count();
        Output {
            backspaces: before.chars().count() - common,
            text: after.chars().skip(common).collect(),
        }
    }
}

/// A case change waiting for the next fragment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Case {
    #[default]
    None,
    Capitalize,
    Lower,
    Upper,
}

/// Where the most recent fragment starts (byte offset, at its space if it
/// has one) and whether a space was written in front of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Fragment {
    start: usize,
    has_space: bool,
}

/// Everything `undo` must restore. Kept as one value so a snapshot taken
/// before `apply` is a complete, exact rollback point.
#[derive(Debug, Clone, PartialEq, Eq)]
struct State {
    text: String,
    attach: bool,
    case: Case,
    glue: bool,
    last: Option<Fragment>,
}

impl Default for State {
    fn default() -> Self {
        // The very first word starts the text, so it is attached.
        State {
            text: String::new(),
            attach: true,
            case: Case::None,
            glue: false,
            last: None,
        }
    }
}

/// One parsed piece of a translation.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Atom {
    Text(String),
    Meta(String),
}

/// Stateful renderer: feed it translations one at a time with [`apply`],
/// take them back with [`undo`].
///
/// [`apply`]: Formatter::apply
/// [`undo`]: Formatter::undo
#[derive(Debug, Clone, Default)]
pub struct Formatter {
    state: State,
    history: Vec<State>,
}

impl Formatter {
    /// An empty formatter with no undo history; the first word it renders
    /// starts the text, so it gets no leading space.
    pub fn new() -> Self {
        Self::default()
    }

    /// Render one dictionary translation on top of what is already there.
    pub fn apply(&mut self, translation: &str) -> Output {
        let before = self.state.clone();
        for atom in parse(translation) {
            match atom {
                Atom::Text(t) => self.state.write(&t, false, false),
                Atom::Meta(m) => self.state.meta(&m),
            }
        }
        let out = Output::diff(&before.text, &self.state.text);
        self.history.push(before);
        out
    }

    /// Revert the most recent un-undone `apply`, text and state alike.
    pub fn undo(&mut self) -> Output {
        match self.history.pop() {
            Some(previous) => {
                let out = Output::diff(&self.state.text, &previous.text);
                self.state = previous;
                out
            }
            None => Output::default(),
        }
    }

    /// Everything rendered so far.
    pub fn text(&self) -> &str {
        &self.state.text
    }
}

/// Render a whole sequence of translations from scratch.
pub fn format(translations: &[&str]) -> String {
    let mut formatter = Formatter::new();
    for t in translations {
        formatter.apply(t);
    }
    formatter.text().to_string()
}

/// Split a translation into text and meta atoms. A bare number is glue, so
/// that consecutive digit strokes build one number.
fn parse(translation: &str) -> Vec<Atom> {
    if !translation.is_empty() && translation.bytes().all(|b| b.is_ascii_digit()) {
        return vec![Atom::Meta(format!("&{translation}"))];
    }

    let mut atoms = Vec::new();
    let mut text = String::new();
    let mut chars = translation.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if matches!(chars.peek(), Some('{') | Some('}')) => {
                text.push(chars.next().expect("peeked"));
            }
            '{' => {
                let rest: String = chars.clone().collect();
                match rest.find('}') {
                    Some(end) => {
                        push_text(&mut atoms, &mut text);
                        atoms.push(Atom::Meta(rest[..end].to_string()));
                        // Skip the meta body and its closing brace.
                        for _ in 0..rest[..end].chars().count() + 1 {
                            chars.next();
                        }
                    }
                    // An unclosed brace cannot start a meta; keep it as text.
                    None => text.push(c),
                }
            }
            _ => text.push(c),
        }
    }
    push_text(&mut atoms, &mut text);
    atoms
}

fn push_text(atoms: &mut Vec<Atom>, text: &mut String) {
    let trimmed = text.trim_matches(' ');
    if !trimmed.is_empty() {
        atoms.push(Atom::Text(trimmed.to_string()));
    }
    text.clear();
}

impl State {
    /// Write one fragment. `attached` is the atom's own wish to join the
    /// previous text; `glue` is whether this atom is glue.
    fn write(&mut self, t: &str, attached: bool, glue: bool) {
        let start = self.text.len();
        let has_space = !(self.attach || attached);
        if has_space {
            self.text.push(' ');
        }
        self.text.push_str(&apply_case(t, self.case));
        self.case = Case::None;
        self.last = Some(Fragment { start, has_space });
        self.attach = false;
        self.glue = glue;
    }

    fn meta(&mut self, meta: &str) {
        match meta {
            "^" | "^^" => {
                self.attach = true;
                self.case = Case::None;
                self.glue = false;
            }
            "" => {
                self.attach = false;
                self.case = Case::None;
                self.glue = false;
            }
            "-|" => self.case = Case::Capitalize,
            ">" => self.case = Case::Lower,
            "<" => self.case = Case::Upper,
            "*-|" => self.retro_case(Case::Capitalize),
            "*>" => self.retro_case(Case::Lower),
            "*<" => self.retro_case(Case::Upper),
            "*!" => self.retro_join(),
            "*?" => self.retro_split(),
            "." | "?" | "!" => {
                self.write(meta, true, false);
                self.case = Case::Capitalize;
            }
            "," | ":" | ";" => self.write(meta, true, false),
            _ => {
                if let Some(x) = meta.strip_prefix('&').filter(|x| !x.is_empty()) {
                    let attached = self.glue;
                    self.write(x, attached, true);
                } else if let Some(x) = meta
                    .strip_prefix('^')
                    .and_then(|m| m.strip_suffix('^'))
                    .filter(|x| !x.is_empty())
                {
                    self.write(x, true, false);
                    self.attach = true;
                } else if let Some(x) = meta.strip_prefix('^').filter(|x| !x.is_empty()) {
                    self.write(x, true, false);
                } else if let Some(x) = meta.strip_suffix('^').filter(|x| !x.is_empty()) {
                    self.write(x, false, false);
                    self.attach = true;
                }
                // Anything else (key presses, engine commands, unknown metas)
                // renders nothing and leaves the state alone.
            }
        }
    }

    /// Re-case the trailing non-whitespace run of the text in place.
    fn retro_case(&mut self, case: Case) {
        let word_start = self
            .text
            .char_indices()
            .rev()
            .take_while(|(_, c)| !c.is_whitespace())
            .last()
            .map(|(i, _)| i);
        let Some(word_start) = word_start else {
            return;
        };

        // Casing can change byte lengths, so a fragment start inside the word
        // must be remapped while the word is rebuilt char by char.
        let fragment_start = self.last.map(|f| f.start);
        let mut new_start = fragment_start;
        let mut word = String::new();
        for (n, (i, c)) in self.text[word_start..].char_indices().enumerate() {
            if fragment_start == Some(word_start + i) {
                new_start = Some(word_start + word.len());
            }
            let first = n == 0;
            match case {
                Case::Capitalize if first => word.extend(c.to_uppercase()),
                Case::Lower if first => word.extend(c.to_lowercase()),
                Case::Upper => word.extend(c.to_uppercase()),
                _ => word.push(c),
            }
        }
        self.text.replace_range(word_start.., &word);
        if let (Some(last), Some(start)) = (self.last.as_mut(), new_start) {
            last.start = start;
        }
    }

    // `retro_join` / `retro_split` operate on the rendered text (the space in
    // front of the last fragment), deliberately unlike Plover's retro macros,
    // which rewrite previous translations: that would need a translation
    // history this renderer does not keep beyond undo snapshots.
    fn retro_join(&mut self) {
        if let Some(last) = self.last.as_mut().filter(|f| f.has_space) {
            self.text.remove(last.start);
            last.has_space = false;
        }
    }

    fn retro_split(&mut self) {
        if let Some(last) = self.last.as_mut().filter(|f| !f.has_space) {
            self.text.insert(last.start, ' ');
            last.has_space = true;
        }
    }
}

fn apply_case(t: &str, case: Case) -> String {
    let mut chars = t.chars();
    match case {
        Case::None => t.to_string(),
        Case::Capitalize => chars
            .next()
            .map(|c| c.to_uppercase().chain(chars).collect())
            .unwrap_or_default(),
        Case::Lower => chars
            .next()
            .map(|c| c.to_lowercase().chain(chars).collect())
            .unwrap_or_default(),
        Case::Upper => {
            let (word, rest) = t.split_at(t.find(' ').unwrap_or(t.len()));
            word.to_uppercase() + rest
        }
    }
}
