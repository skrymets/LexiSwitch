//! Changing the case of selected text, and stripping what the user does not want in it.
//!
//! Pure logic, no Windows. The window draws what this produces and decides nothing.
//!
//! Every transform is defined on a **word sequence**, not a character sequence: split into
//! words and separators, strip, resolve the separators, apply the style, join. That is what
//! makes `Words Capitalized` with spaces removed give `HelloWorld` rather than `Helloworld` —
//! the word boundary survives even though the space character does not. A chain of
//! character-level rewrites cannot express that without the order of the rewrites deciding the
//! answer.

use lexiswitch_core::casing::{generate, render, toggle_affects, Lines, Strips, Style, Toggle, Whitespace};

fn plain() -> Strips {
    Strips::default()
}

fn only(style: Style, text: &str) -> String {
    render(text, style, &plain())
}

// --- segmentation --------------------------------------------------------------------

/// Whitespace, and the three characters an identifier is normally written with, all begin a
/// new word.
#[test]
fn words_break_at_whitespace_and_identifier_separators() {
    for text in ["one two", "one_two", "one-two", "one.two"] {
        assert_eq!(
            render(text, Style::Snake, &plain()),
            "one_two",
            "{text:?} did not split into two words"
        );
    }
}

/// A lowercase letter followed by an uppercase one is a boundary with no character to show
/// for it, which is how an existing identifier is re-cased rather than mangled.
#[test]
fn a_hump_is_a_word_boundary() {
    assert_eq!(only(Style::Snake, "getUserName"), "get_user_name");
    assert_eq!(only(Style::KebabLower, "getUserName"), "get-user-name");
}

/// A digit beside a letter is **not** a boundary. Splitting there turns `getUserV2` into
/// `get_user_v_2`, which is not what anybody means.
#[test]
fn a_digit_does_not_start_a_new_word() {
    assert_eq!(only(Style::Snake, "getUserV2"), "get_user_v2");
    assert_eq!(only(Style::Camel, "user_name_2"), "userName2");
}

/// Consecutive uppercase letters are one word. An acronym has no boundary to find, so
/// `HTTPServer` stays whole — a limitation worth pinning down rather than discovering.
#[test]
fn an_acronym_is_one_word() {
    assert_eq!(only(Style::Snake, "HTTPServer"), "httpserver");
}

// --- the styles ----------------------------------------------------------------------

/// Every style, against one input, so the whole set can be read at once and compared.
#[test]
fn every_style_against_one_phrase() {
    let text = "hello World 42 Bye";
    let expected = [
        (Style::AsTyped, "hello World 42 Bye"),
        (Style::Invert, "HELLO wORLD 42 bYE"),
        (Style::Lower, "hello world 42 bye"),
        (Style::Upper, "HELLO WORLD 42 BYE"),
        (Style::Camel, "helloWorld42Bye"),
        (Style::Snake, "hello_world_42_bye"),
        (Style::Dot, "hello.world.42.bye"),
        (Style::KebabLower, "hello-world-42-bye"),
        (Style::Pascal, "HelloWorld42Bye"),
        (Style::ScreamingSnake, "HELLO_WORLD_42_BYE"),
        (Style::CapitalizedSnake, "Hello_World_42_Bye"),
        (Style::KebabUpper, "HELLO-WORLD-42-BYE"),
        (Style::Sentence, "Hello world 42 bye"),
        (Style::Title, "Hello World 42 Bye"),
    ];
    for (style, want) in expected {
        assert_eq!(render(text, style, &plain()), want, "{}", style.label());
    }
    assert_eq!(
        expected.len(),
        Style::ALL.len(),
        "a style was added without a case here"
    );
}

/// `As typed` earns its place by changing nothing at all, so the strip options are usable
/// without accepting a case change nobody asked for.
#[test]
fn as_typed_changes_no_letter() {
    let text = "hello, World 42! Bye.";
    assert_eq!(only(Style::AsTyped, text), text);
}

/// The prose styles preserve the separators they found. Only the identifier styles are
/// entitled to replace them.
#[test]
fn prose_styles_keep_the_original_separators() {
    let text = "hello,   World\tBye.";
    assert_eq!(only(Style::Lower, text), "hello,   world\tbye.");
    assert_eq!(only(Style::Upper, text), "HELLO,   WORLD\tBYE.");
}

/// An identifier cannot hold a comma or an exclamation mark, so identifier styles drop them
/// whatever the strip options say. The destruction is not hidden — the window shows the result
/// before anything is applied.
#[test]
fn identifier_styles_drop_punctuation_unconditionally() {
    let text = "hello, World! Bye.";
    assert_eq!(only(Style::Camel, text), "helloWorldBye");
    assert_eq!(only(Style::Snake, text), "hello_world_bye");
}

/// Ukrainian is the language this program exists for; case has to work in it.
#[test]
fn ukrainian_changes_case() {
    let text = "привіт світ";
    assert_eq!(only(Style::Upper, text), "ПРИВІТ СВІТ");
    assert_eq!(only(Style::Title, text), "Привіт Світ");
    assert_eq!(only(Style::Camel, text), "привітСвіт");
}

/// German `ß` uppercases to two characters. The result is longer than the input, which is the
/// case a positional preview window gets wrong and a length-tolerant one does not.
#[test]
fn eszett_grows_when_uppercased() {
    assert_eq!(only(Style::Upper, "straße"), "STRASSE");
    assert_eq!(only(Style::Title, "die straße"), "Die Straße");
}

/// Capitalising means first character up, rest down — not "leave the rest alone", which would
/// make `Title` a no-op on text that is already shouting.
#[test]
fn capitalising_lowercases_the_remainder() {
    assert_eq!(only(Style::Title, "HELLO WORLD"), "Hello World");
    assert_eq!(only(Style::Sentence, "HELLO WORLD"), "Hello world");
    assert_eq!(only(Style::Pascal, "HELLO WORLD"), "HelloWorld");
}

/// Inverting is per character and is its own inverse.
#[test]
fn inverting_twice_returns_the_original() {
    let text = "Hello, World 42!";
    let once = only(Style::Invert, text);
    assert_ne!(once, text);
    assert_eq!(only(Style::Invert, &once), text);
}

/// A multi-line selection keeps its lines unless a line option says otherwise.
#[test]
fn line_breaks_survive_by_default() {
    assert_eq!(only(Style::Upper, "one\ntwo"), "ONE\nTWO");
}

/// Nothing to work with is not an error.
#[test]
fn degenerate_inputs_do_not_panic() {
    for text in ["", " ", "\n", "!!!", "___", "42"] {
        for style in Style::ALL {
            let _ = render(text, style, &plain());
        }
    }
}

// --- the strip options ---------------------------------------------------------------

/// Punctuation goes from inside words and from the separators, but whitespace stays: that is
/// what makes it *punctuation* stripping rather than a general purge.
#[test]
fn stripping_punctuation_leaves_whitespace_alone() {
    let strips = Strips {
        punctuation: true,
        ..plain()
    };
    assert_eq!(
        render("hello, World 42! Bye.", Style::AsTyped, &strips),
        "hello World 42 Bye"
    );
}

/// Digits go from inside words — and only digits. A word emptied of them leaves its spaces
/// behind, because the digits option removes digits and nothing else. Anything cleverer would
/// be an option that silently does two things, and `normalize` already does the second one.
#[test]
fn stripping_digits_removes_digits_and_nothing_else() {
    let strips = Strips {
        digits: true,
        ..plain()
    };
    assert_eq!(render("v2 build 3", Style::AsTyped, &strips), "v build ");
    assert_eq!(render("a 42 b", Style::AsTyped, &strips), "a  b");
    assert_eq!(render("user2name", Style::Snake, &strips), "username");
}

/// The gap an emptied word leaves is the whitespace option's business, and the two compose to
/// give what someone stripping digits from a phrase actually wants.
#[test]
fn stripping_digits_composes_with_normalizing() {
    let strips = Strips {
        digits: true,
        spaces: Whitespace::Normalize,
        ..plain()
    };
    assert_eq!(render("a 42 b", Style::AsTyped, &strips), "a b");
}

/// An identifier style has to drop the emptied word outright: keeping it would double the
/// delimiter around the gap.
#[test]
fn an_emptied_word_does_not_double_an_identifier_delimiter() {
    let strips = Strips {
        digits: true,
        ..plain()
    };
    assert_eq!(render("a 42 b", Style::Snake, &strips), "a_b");
}

/// A word emptied by stripping must not take the first-word position with it, or `camelCase`
/// capitalises the head it is supposed to leave lowercase.
#[test]
fn an_emptied_leading_word_does_not_take_the_first_position() {
    let strips = Strips {
        digits: true,
        ..plain()
    };
    assert_eq!(render("42 hello world", Style::Camel, &strips), "helloWorld");
}

/// The heart of the word model: the boundary survives the character that made it.
#[test]
fn removing_spaces_keeps_the_word_boundary() {
    let strips = Strips {
        spaces: Whitespace::Remove,
        ..plain()
    };
    assert_eq!(render("Hello World", Style::Title, &strips), "HelloWorld");
    assert_eq!(render("Hello World", Style::Lower, &strips), "helloworld");
}

/// Normalizing collapses runs and turns tabs into spaces, and does not trim: trimming is a
/// different operation, and `remove` is there for anyone who wants it.
#[test]
fn normalizing_collapses_runs_and_converts_tabs() {
    let strips = Strips {
        spaces: Whitespace::Normalize,
        ..plain()
    };
    assert_eq!(
        render("hello   world\tbye", Style::AsTyped, &strips),
        "hello world bye"
    );
    assert_eq!(render(" hello  world ", Style::AsTyped, &strips), " hello world ");
}

/// Joining puts a space where the break was; removing puts nothing there.
#[test]
fn the_two_line_options_differ() {
    let join = Strips {
        lines: Lines::Join,
        ..plain()
    };
    let remove = Strips {
        lines: Lines::Remove,
        ..plain()
    };
    assert_eq!(render("one\ntwo", Style::AsTyped, &join), "one two");
    assert_eq!(render("one\ntwo", Style::AsTyped, &remove), "onetwo");
}

/// A Windows line ending is one break, not two.
#[test]
fn a_carriage_return_pair_is_one_break() {
    let join = Strips {
        lines: Lines::Join,
        ..plain()
    };
    assert_eq!(render("one\r\ntwo", Style::AsTyped, &join), "one two");
}

/// Lines resolve before horizontal whitespace, so a joined break becomes a space that
/// normalizing then governs. Without that order this leaves three spaces.
#[test]
fn joining_then_normalizing_yields_exactly_one_space() {
    let strips = Strips {
        spaces: Whitespace::Normalize,
        lines: Lines::Join,
        ..plain()
    };
    assert_eq!(
        render("one \n two", Style::AsTyped, &strips),
        "one two",
        "the line option must resolve before the whitespace option"
    );
}

/// Removing whitespace makes the two line options agree, which is a real property and the
/// reason one of the pair dims when the other is redundant.
#[test]
fn removing_spaces_makes_the_line_options_agree() {
    let text = "one \n two";
    let join = Strips {
        spaces: Whitespace::Remove,
        lines: Lines::Join,
        ..plain()
    };
    let remove = Strips {
        spaces: Whitespace::Remove,
        lines: Lines::Remove,
        ..plain()
    };
    assert_eq!(
        render(text, Style::AsTyped, &join),
        render(text, Style::AsTyped, &remove)
    );
}

// --- exclusivity ---------------------------------------------------------------------

/// The pairs are two three-valued settings, so exclusivity is structural. Setting one member
/// moves the setting; it cannot stack with the other.
#[test]
fn setting_one_member_of_a_pair_clears_the_other() {
    let mut s = plain();
    s.apply(Toggle::RemoveSpaces);
    assert_eq!(s.spaces, Whitespace::Remove);
    s.apply(Toggle::NormalizeSpaces);
    assert_eq!(s.spaces, Whitespace::Normalize);

    s.apply(Toggle::JoinLines);
    assert_eq!(s.lines, Lines::Join);
    s.apply(Toggle::RemoveLines);
    assert_eq!(s.lines, Lines::Remove);
}

/// Pressing the same accelerator twice turns the option back off, so every toggle is its own
/// undo and no option can be reached but not left.
#[test]
fn applying_the_same_toggle_twice_clears_it() {
    for toggle in Toggle::ALL {
        let mut s = plain();
        s.apply(toggle);
        assert!(s.is_set(toggle), "{toggle:?} did not set");
        s.apply(toggle);
        assert!(!s.is_set(toggle), "{toggle:?} did not clear");
        assert_eq!(s, plain(), "{toggle:?} left something behind");
    }
}

/// Exactly one member of each pair can read as set at a time.
#[test]
fn a_pair_never_reads_as_both_set() {
    let mut s = plain();
    s.apply(Toggle::RemoveSpaces);
    assert!(s.is_set(Toggle::RemoveSpaces));
    assert!(!s.is_set(Toggle::NormalizeSpaces));
    s.apply(Toggle::NormalizeSpaces);
    assert!(!s.is_set(Toggle::RemoveSpaces));
    assert!(s.is_set(Toggle::NormalizeSpaces));
}

// --- the offered rows ----------------------------------------------------------------

/// Every style is offered every time, in the declared order. The list is what the user aims
/// at, so it does not change shape when the text or the options do — which is the opposite of
/// what layout candidates do, and deliberately so.
#[test]
fn every_style_is_always_offered_in_order() {
    for text in ["", "hello", "ALREADY UPPER", "hello, World 42! Bye."] {
        let rows = generate(text, &plain());
        assert_eq!(rows.len(), Style::ALL.len(), "{text:?} changed the list length");
        for (row, style) in rows.iter().zip(Style::ALL) {
            assert_eq!(row.style, style, "{text:?} changed the order");
        }
    }
}

/// A row that would change nothing is still offered, and says so. Dropping it is what the
/// layout chooser does, and it would reshuffle this list on every keystroke.
#[test]
fn a_row_that_changes_nothing_is_marked_not_removed() {
    let rows = generate("HELLO", &plain());
    let upper = rows.iter().find(|r| r.style == Style::Upper).expect("offered");
    assert!(!upper.changes, "UPPERCASE of shouting text changes nothing");
    let lower = rows.iter().find(|r| r.style == Style::Lower).expect("offered");
    assert!(lower.changes);
}

/// `As typed` changes nothing until a strip option gives it something to do.
#[test]
fn as_typed_only_changes_something_once_a_strip_is_on() {
    let rows = generate("hello, world", &plain());
    let row = rows.iter().find(|r| r.style == Style::AsTyped).unwrap();
    assert!(!row.changes);

    let rows = generate(
        "hello, world",
        &Strips {
            punctuation: true,
            ..plain()
        },
    );
    let row = rows.iter().find(|r| r.style == Style::AsTyped).unwrap();
    assert!(row.changes);
    assert_eq!(row.text, "hello world");
}

/// Duplicate results are kept. Two styles that agree on this text are still two different
/// choices, and collapsing them would make the list jump about.
#[test]
fn styles_that_agree_are_both_kept() {
    let rows = generate("hello", &plain());
    let upper: Vec<&str> = rows
        .iter()
        .filter(|r| r.text == "HELLO")
        .map(|r| r.style.label())
        .collect();
    assert!(
        upper.len() > 1,
        "expected several styles to agree on HELLO, got {upper:?}"
    );
    assert_eq!(rows.len(), Style::ALL.len());
}

// --- the dim rule --------------------------------------------------------------------

/// A toggle reports no effect when flipping it would produce the same text for the selected
/// style. One rule, computed rather than declared, so a new style needs no rules table entry.
#[test]
fn a_toggle_with_no_effect_says_so() {
    // Nothing to remove.
    assert!(!toggle_affects("hello world", Style::Lower, &plain(), Toggle::Digits));
    // Something to remove.
    assert!(toggle_affects("hello world2", Style::Lower, &plain(), Toggle::Digits));
}

/// An identifier style joins with its own delimiter, so the whitespace options cannot reach it.
#[test]
fn whitespace_options_do_not_affect_identifier_styles() {
    for toggle in [Toggle::RemoveSpaces, Toggle::NormalizeSpaces] {
        assert!(
            !toggle_affects("hello   world", Style::Snake, &plain(), toggle),
            "{toggle:?} should not reach snake_case"
        );
    }
    assert!(toggle_affects("hello   world", Style::Lower, &plain(), Toggle::NormalizeSpaces));
}

/// With whitespace removed, the two line options produce the same text, so the redundant one
/// reports no effect. This falls out of the rule rather than being a special case.
#[test]
fn a_redundant_member_of_a_pair_dims() {
    let strips = Strips {
        spaces: Whitespace::Remove,
        lines: Lines::Join,
        ..plain()
    };
    assert!(
        !toggle_affects("one \n two", Style::AsTyped, &strips, Toggle::RemoveLines),
        "with spaces removed, joining and removing breaks agree"
    );
}

/// Punctuation cannot reach an identifier style, which already drops it.
#[test]
fn punctuation_does_not_affect_identifier_styles() {
    assert!(!toggle_affects("hello, world!", Style::Camel, &plain(), Toggle::Punctuation));
    assert!(toggle_affects("hello, world!", Style::Lower, &plain(), Toggle::Punctuation));
}

// --- identity ------------------------------------------------------------------------

/// Every style has a stable identifier for the usage file and a distinct label for the window.
#[test]
fn styles_have_distinct_stable_identities() {
    let mut ids: Vec<&str> = Style::ALL.iter().map(|s| s.id()).collect();
    let mut labels: Vec<&str> = Style::ALL.iter().map(|s| s.label()).collect();
    let count = Style::ALL.len();
    ids.sort();
    ids.dedup();
    labels.sort();
    labels.dedup();
    assert_eq!(ids.len(), count, "two styles share an identifier");
    assert_eq!(labels.len(), count, "two styles share a label");
    for style in Style::ALL {
        assert_eq!(Style::from_id(style.id()), Some(style));
    }
}

/// An identifier this version does not know is ignored, not fatal: a state file written by a
/// later version must not stop this one starting.
#[test]
fn an_unknown_identifier_is_declined() {
    assert_eq!(Style::from_id("morse_code"), None);
}

/// The three groups are the user's own grouping, and every style is in exactly one.
#[test]
fn the_groups_partition_the_styles() {
    let mut seen = Vec::new();
    for group in Style::GROUPS {
        assert!(!group.is_empty());
        seen.extend_from_slice(group);
    }
    assert_eq!(seen, Style::ALL.to_vec(), "the groups are not the list, in order");
}
