use steno_core::{format, Formatter, Output};

fn out(backspaces: usize, text: &str) -> Output {
    Output {
        backspaces,
        text: text.to_string(),
    }
}

/// Apply an `Output` to a buffer the way a keyboard would.
fn replay(buffer: &mut String, o: &Output) {
    for _ in 0..o.backspaces {
        buffer.pop().expect("backspace past start of buffer");
    }
    buffer.push_str(&o.text);
}

#[test]
fn plain_words_are_space_separated() {
    assert_eq!(format(&["hello", "world"]), "hello world");
    assert_eq!(format(&["hello world"]), "hello world");
    assert_eq!(format(&[]), "");
}

#[test]
fn text_atoms_drop_outer_spaces() {
    assert_eq!(format(&["  hello ", " world  "]), "hello world");
    assert_eq!(format(&["a", "   ", "b"]), "a b");
}

#[test]
fn attach_operators() {
    assert_eq!(format(&["test", "{^ing}"]), "testing");
    assert_eq!(format(&["make", "{^ing}"]), "makeing");
    assert_eq!(format(&["{in^}", "side"]), "inside");
    assert_eq!(format(&["a", "{^-^}", "b"]), "a-b");
    assert_eq!(format(&["a", "{^}", "b"]), "ab");
    assert_eq!(format(&["a", "{^^}", "b"]), "ab");
    assert_eq!(format(&["x{^}", "y"]), "xy");
    assert_eq!(format(&["x", "{^}y"]), "xy");
    assert_eq!(format(&["a", "{^ ^}", "b"]), "a b");
    assert_eq!(format(&["a", "{^ ^}", "{^ ^}", "b"]), "a  b");
}

#[test]
fn attach_drops_pending_case() {
    assert_eq!(format(&["a", "{-|}", "{^}", "b"]), "ab");
}

#[test]
fn glue() {
    assert_eq!(format(&["{&a}", "{&b}", "{&c}"]), "abc");
    assert_eq!(format(&["1", "2", "3"]), "123");
    assert_eq!(format(&["page", "4", "2"]), "page 42");
    assert_eq!(format(&["{&a}", "word", "{&b}"]), "a word b");
    assert_eq!(format(&["4", "{&x}"]), "4x");
}

#[test]
fn case_operators() {
    assert_eq!(format(&["{-|}", "hello"]), "Hello");
    assert_eq!(format(&["{<}", "nasa"]), "NASA");
    assert_eq!(format(&["{<}", "nasa rocks"]), "NASA rocks");
    assert_eq!(format(&["{>}", "Word"]), "word");
    assert_eq!(format(&["{-|}", "{>}", "Word"]), "word");
    assert_eq!(format(&["a", "{-|}", "b"]), "a B");
    assert_eq!(format(&["{-|}", "über"]), "Über");
}

#[test]
fn case_operators_keep_attach() {
    assert_eq!(format(&["a", "{^}", "{-|}", "b"]), "aB");
}

#[test]
fn retro_case_operators() {
    assert_eq!(format(&["hello", "{*-|}"]), "Hello");
    assert_eq!(format(&["a", "nasa", "{*<}"]), "a NASA");
    assert_eq!(format(&["a", "Word", "{*>}"]), "a word");
    assert_eq!(format(&["test", "{^ing}", "{*<}"]), "TESTING");
    assert_eq!(format(&["{*-|}", "x"]), "x");
    assert_eq!(format(&["a", "{^ ^}", "{*<}"]), "a ");
}

#[test]
fn retro_case_keeps_other_state() {
    // Pending case survives a retro case.
    assert_eq!(format(&["a", "{-|}", "{*<}", "b"]), "A B");
    // Attach survives a retro case.
    assert_eq!(format(&["{a^}", "{*<}", "b"]), "Ab");
}

#[test]
fn punctuation() {
    assert_eq!(format(&["hello", "{.}", "world"]), "hello. World");
    assert_eq!(format(&["what", "{?}", "no"]), "what? No");
    assert_eq!(format(&["wow", "{!}", "ok"]), "wow! Ok");
    assert_eq!(format(&["yes", "{,}", "sir"]), "yes, sir");
    assert_eq!(format(&["a", "{:}", "b", "{;}", "c"]), "a: b; c");
}

#[test]
fn retro_spacing() {
    assert_eq!(format(&["hello", "world", "{*!}"]), "helloworld");
    assert_eq!(format(&["red", "fish tank", "{*!}"]), "redfish tank");
    assert_eq!(format(&["in", "{^side}", "{*?}"]), "in side");
    assert_eq!(format(&["p", "q", "{*!}", "{*?}"]), "p q");
    assert_eq!(format(&["p", "q", "{*!}", "{*!}"]), "pq");
    assert_eq!(format(&["left", "{*?}", "right"]), " left right");
    assert_eq!(format(&["{*!}", "{*?}", "a"]), "a");
}

#[test]
fn retro_spacing_after_retro_case() {
    assert_eq!(format(&["x", "straße", "{*<}", "{*!}"]), "xSTRASSE");
    assert_eq!(format(&["in", "{^side}", "{*<}", "{*?}"]), "IN SIDE");
}

#[test]
fn glue_is_cleared_only_by_fragments_and_cancel() {
    assert_eq!(format(&["1", "{}", "2"]), "1 2");
    assert_eq!(format(&["1", "{,}", "2"]), "1, 2");
    assert_eq!(format(&["1", "{-|}", "2"]), "12");
    assert_eq!(format(&["1", "{#Return}", "2"]), "12");
    assert_eq!(format(&["1", "2", "{*?}", "3"]), "1 23");
    assert_eq!(format(&["a{^}", "{&b}"]), "ab");
}

#[test]
fn retro_spacing_keeps_attach_and_case() {
    assert_eq!(format(&["in", "{^side}", "{*?}", "{*?}"]), "in side");
    assert_eq!(format(&["a", "b{^}", "{*!}", "c"]), "abc");
    assert_eq!(format(&["a", "b", "{-|}", "{*!}", "c"]), "ab C");
}

#[test]
fn pending_case_is_consumed_by_any_fragment() {
    assert_eq!(format(&["a", "{-|}", "{,}", "b"]), "a, b");
    assert_eq!(format(&["{-|}", "hello world"]), "Hello world");
}

#[test]
fn several_atoms_in_one_translation() {
    assert_eq!(format(&["{-|}hello{.}", "world"]), "Hello. World");
    assert_eq!(format(&["a", "{^}b{^}", "c"]), "abc");
}

#[test]
fn only_all_digit_translations_are_glue() {
    assert_eq!(format(&["1", "2a"]), "1 2a");
}

#[test]
fn undo_restores_has_space_flag() {
    let mut f = Formatter::new();
    f.apply("a");
    f.apply("b");
    f.apply("{*!}");
    assert_eq!(f.undo(), out(1, " b"));
    f.apply("{*!}");
    assert_eq!(f.text(), "ab");
}

#[test]
fn output_for_leading_space_insert() {
    let mut f = Formatter::new();
    f.apply("left");
    assert_eq!(f.apply("{*?}"), out(4, " left"));
}

#[test]
fn cancel_escapes_and_unknown_metas() {
    assert_eq!(format(&["a", "{-|}", "{}", "word"]), "a word");
    assert_eq!(format(&["a{^}", "{}", "b"]), "a b");
    assert_eq!(format(&["\\{x\\}"]), "{x}");
    assert_eq!(format(&["a", "{#Return}", "b"]), "a b");
    assert_eq!(format(&["a{^}", "{PLOVER:TOGGLE}", "b"]), "ab");
    assert_eq!(format(&["a", "{-|}", "{#Return}", "b"]), "a B");
}

#[test]
fn output_is_minimal_char_delta() {
    let mut f = Formatter::new();
    assert_eq!(f.apply("cat"), out(0, "cat"));
    assert_eq!(f.apply("{*-|}"), out(3, "Cat"));
    assert_eq!(f.apply("{.}"), out(0, "."));
    assert_eq!(f.text(), "Cat.");

    let mut f = Formatter::new();
    f.apply("日本");
    assert_eq!(f.apply("{^語}"), out(0, "語"));
    assert_eq!(f.undo(), out(1, ""));
    assert_eq!(f.text(), "日本");
}

#[test]
fn output_for_retro_join() {
    let mut f = Formatter::new();
    f.apply("hello");
    f.apply("world");
    assert_eq!(f.apply("{*!}"), out(6, "world"));
    assert_eq!(f.text(), "helloworld");
}

#[test]
fn no_text_change_gives_empty_output() {
    let mut f = Formatter::default();
    f.apply("a");
    assert_eq!(f.apply("{-|}"), Output::default());
    assert_eq!(f.apply("{#Return}"), Output::default());
}

#[test]
fn undo_restores_text_and_state() {
    let mut f = Formatter::new();
    f.apply("hello");
    f.apply("{.}");
    f.apply("world");
    assert_eq!(f.text(), "hello. World");
    assert_eq!(f.undo(), out(6, ""));
    assert_eq!(f.text(), "hello.");
    // Pending capitalization is back.
    f.apply("there");
    assert_eq!(f.text(), "hello. There");
}

#[test]
fn undo_of_textless_apply_reverts_state() {
    let mut f = Formatter::new();
    f.apply("a");
    f.apply("{^}");
    assert_eq!(f.undo(), Output::default());
    f.apply("b");
    assert_eq!(f.text(), "a b");

    let mut f = Formatter::new();
    f.apply("a");
    f.apply("{-|}");
    f.undo();
    f.apply("b");
    assert_eq!(f.text(), "a b");
}

#[test]
fn undo_restores_glue_and_last_fragment() {
    let mut f = Formatter::new();
    f.apply("1");
    f.apply("x");
    f.undo();
    f.apply("2");
    assert_eq!(f.text(), "12");

    let mut f = Formatter::new();
    f.apply("a");
    f.apply("b");
    f.apply("c");
    f.undo();
    f.apply("{*!}");
    assert_eq!(f.text(), "ab");
}

#[test]
fn undo_with_nothing_left_is_a_no_op() {
    let mut f = Formatter::new();
    assert_eq!(f.undo(), Output::default());
    f.apply("a");
    f.undo();
    assert_eq!(f.undo(), Output::default());
    assert_eq!(f.text(), "");
    // The first word is attached again after undoing everything.
    f.apply("b");
    assert_eq!(f.text(), "b");
}

#[test]
fn repeated_undo_walks_back() {
    let mut f = Formatter::new();
    f.apply("one");
    f.apply("two");
    f.apply("three");
    f.undo();
    f.undo();
    assert_eq!(f.text(), "one");
}

#[test]
fn format_matches_formatter() {
    let ts = ["{-|}", "hello", "{,}", "world", "{.}", "4", "2"];
    let mut f = Formatter::new();
    for t in ts {
        f.apply(t);
    }
    assert_eq!(format(&ts), f.text());
}

#[test]
fn replay_invariant_holds_across_apply_and_undo() {
    enum Step {
        Apply(&'static str),
        Undo,
    }
    use Step::*;
    let steps = [
        Apply("hello"),
        Apply("{.}"),
        Apply("wörld"),
        Apply("{*<}"),
        Undo,
        Apply("{*!}"),
        Apply("{*?}"),
        Apply("日本"),
        Apply("{^語}"),
        Apply("{*-|}"),
        Undo,
        Undo,
        Apply("1"),
        Apply("2"),
        Apply("{-|}"),
        Undo,
        Undo,
        Undo,
        Apply("{^ ^}"),
        Apply("straße"),
        Apply("{*<}"),
        Undo,
        Undo,
        Undo,
        Undo,
        Undo,
        Undo,
        Undo,
        Undo,
        Undo,
        Undo,
        Apply("again"),
    ];
    let mut f = Formatter::new();
    let mut buffer = String::new();
    for step in steps {
        let o = match step {
            Apply(t) => f.apply(t),
            Undo => f.undo(),
        };
        replay(&mut buffer, &o);
        assert_eq!(buffer, f.text());
    }
}
