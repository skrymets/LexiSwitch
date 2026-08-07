//! Changing the case of selected text, and stripping what the user does not want in it.
//!
//! A sibling of layout conversion, not a variant of it
//! ([ADR-0014](../../docs/adr/0014-case-conversion-is-a-separate-transaction.md)): any text at
//! all can be re-cased and any text at all can be reinterpreted under another layout, so
//! offering both in one list would put roughly half the rows in the way of the other half every
//! time, with nothing in the characters to say which half.
//!
//! ## Words, not characters
//!
//! Every transform is defined on a **word sequence**: split into words and separators, strip
//! characters from the words, resolve the separators, apply the style, join.
//!
//! The alternative is a chain of character-level rewrites, and it cannot express the one
//! property that matters most. `Words Capitalized` with spaces removed must give `HelloWorld`,
//! not `Helloworld` — the word *boundary* has to survive the space *character* that produced
//! it. Removing spaces first destroys the information the style needs; removing them afterwards
//! requires the style to have kept information it has already thrown away. Making the boundary
//! a thing in its own right removes the ambiguity rather than resolving it case by case.
//!
//! The two separator steps are ordered: line breaks first, horizontal whitespace second. So a
//! break joined into a space is then governed by the whitespace option, and `join` with
//! `normalize` produces one space rather than three.
//!
//! ## Nothing is filtered
//!
//! [`generate`] returns every style, always, in the declared order, each carrying whether it
//! would change the text. This is deliberately the opposite of `candidates::generate`, which
//! drops conversions that change nothing. That is right for a candidate set that is discovered
//! and unnamed; here the set is fixed and every row has a name the user aims at, so a list that
//! changed shape whenever an option was pressed would destroy the muscle memory the window's
//! promoted entries exist to build.

/// One way of re-casing the selection.
///
/// The order is the order they are shown in, and it is the user's own grouping: the plain case
/// operations, then the identifier styles, then the sentence styles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    AsTyped,
    Invert,
    Lower,
    Upper,
    Camel,
    Snake,
    Dot,
    KebabLower,
    Pascal,
    ScreamingSnake,
    CapitalizedSnake,
    KebabUpper,
    Sentence,
    Title,
}

impl Style {
    /// Every style, in the order they are offered.
    pub const ALL: [Style; 14] = [
        Style::AsTyped,
        Style::Invert,
        Style::Lower,
        Style::Upper,
        Style::Camel,
        Style::Snake,
        Style::Dot,
        Style::KebabLower,
        Style::Pascal,
        Style::ScreamingSnake,
        Style::CapitalizedSnake,
        Style::KebabUpper,
        Style::Sentence,
        Style::Title,
    ];

    /// The same styles, grouped as they are drawn. Concatenating the groups gives [`Style::ALL`].
    pub const GROUPS: [&'static [Style]; 3] = [
        &[Style::AsTyped, Style::Invert, Style::Lower, Style::Upper],
        &[
            Style::Camel,
            Style::Snake,
            Style::Dot,
            Style::KebabLower,
            Style::Pascal,
            Style::ScreamingSnake,
            Style::CapitalizedSnake,
            Style::KebabUpper,
        ],
        &[Style::Sentence, Style::Title],
    ];

    /// What the window calls it.
    pub fn label(&self) -> &'static str {
        match self {
            Style::AsTyped => "As typed",
            Style::Invert => "Invert case",
            Style::Lower => "lowercase",
            Style::Upper => "UPPERCASE",
            Style::Camel => "camelCase",
            Style::Snake => "snake_case",
            Style::Dot => "dot.case",
            Style::KebabLower => "kebab-lowercase",
            Style::Pascal => "PascalCase",
            Style::ScreamingSnake => "SCREAMING_SNAKE_CASE",
            Style::CapitalizedSnake => "Capitalized_Snake_Case",
            Style::KebabUpper => "KEBAB-UPPERCASE",
            Style::Sentence => "First word capitalized",
            Style::Title => "Words Capitalized",
        }
    }

    /// What the usage file calls it.
    ///
    /// Separate from [`label`](Style::label) on purpose: the label is prose and may be reworded,
    /// while this is written to disk and must not change under a stored count.
    pub fn id(&self) -> &'static str {
        match self {
            Style::AsTyped => "as_typed",
            Style::Invert => "invert",
            Style::Lower => "lower",
            Style::Upper => "upper",
            Style::Camel => "camel",
            Style::Snake => "snake",
            Style::Dot => "dot",
            Style::KebabLower => "kebab_lower",
            Style::Pascal => "pascal",
            Style::ScreamingSnake => "screaming_snake",
            Style::CapitalizedSnake => "capitalized_snake",
            Style::KebabUpper => "kebab_upper",
            Style::Sentence => "sentence",
            Style::Title => "title",
        }
    }

    /// The style an identifier names, or `None` if this version has no such style.
    ///
    /// `None` is not an error: a usage file written by a later version names styles this one
    /// does not have, and the right response is to ignore them rather than refuse to start.
    pub fn from_id(id: &str) -> Option<Style> {
        Style::ALL.into_iter().find(|s| s.id() == id)
    }

    /// Whether this style produces an identifier, and so supplies its own delimiter and drops
    /// everything that is not a letter or a digit.
    fn is_identifier(&self) -> bool {
        matches!(
            self,
            Style::Camel
                | Style::Snake
                | Style::Dot
                | Style::KebabLower
                | Style::Pascal
                | Style::ScreamingSnake
                | Style::CapitalizedSnake
                | Style::KebabUpper
        )
    }

    /// What an identifier style joins its words with.
    fn delimiter(&self) -> &'static str {
        match self {
            Style::Snake | Style::ScreamingSnake | Style::CapitalizedSnake => "_",
            Style::Dot => ".",
            Style::KebabLower | Style::KebabUpper => "-",
            _ => "",
        }
    }
}

/// What happens to horizontal whitespace between words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Whitespace {
    #[default]
    Keep,
    /// Every run becomes one space, and a tab becomes a space. Does not trim: trimming is a
    /// different operation, and `Remove` is available to anyone who wants it.
    Normalize,
    Remove,
}

/// What happens to line breaks between words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lines {
    #[default]
    Keep,
    /// The break becomes a space, which the whitespace option then governs.
    Join,
    Remove,
}

/// What the user has asked to be taken out of the text.
///
/// Two booleans and two three-valued settings. The three-valued ones are the mutually exclusive
/// pairs the window draws as four checkboxes: modelling them as four booleans would make
/// exclusivity a rule to enforce rather than a thing that is true.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Strips {
    pub punctuation: bool,
    pub digits: bool,
    pub spaces: Whitespace,
    pub lines: Lines,
}

/// One thing an accelerator key turns on or off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toggle {
    Punctuation,
    Digits,
    RemoveSpaces,
    NormalizeSpaces,
    JoinLines,
    RemoveLines,
}

impl Toggle {
    /// Every toggle, in the order the window numbers them.
    pub const ALL: [Toggle; 6] = [
        Toggle::Punctuation,
        Toggle::Digits,
        Toggle::RemoveSpaces,
        Toggle::NormalizeSpaces,
        Toggle::JoinLines,
        Toggle::RemoveLines,
    ];

    /// What the window calls it.
    pub fn label(&self) -> &'static str {
        match self {
            Toggle::Punctuation => "punctuation",
            Toggle::Digits => "digits",
            Toggle::RemoveSpaces => "remove",
            Toggle::NormalizeSpaces => "normalize",
            Toggle::JoinLines => "join with a space",
            Toggle::RemoveLines => "remove",
        }
    }
}

impl Strips {
    /// Turn a toggle on, or off again if it is already on.
    ///
    /// Setting one member of a pair moves the setting rather than stacking with the other, which
    /// is what makes the pairs exclusive without a rule saying so. Pressing the same accelerator
    /// twice returns to `Keep`, so every option is its own undo and none can be reached but not
    /// left.
    pub fn apply(&mut self, toggle: Toggle) {
        match toggle {
            Toggle::Punctuation => self.punctuation = !self.punctuation,
            Toggle::Digits => self.digits = !self.digits,
            Toggle::RemoveSpaces => {
                self.spaces = pick(self.spaces, Whitespace::Remove, Whitespace::Keep)
            }
            Toggle::NormalizeSpaces => {
                self.spaces = pick(self.spaces, Whitespace::Normalize, Whitespace::Keep)
            }
            Toggle::JoinLines => self.lines = pick(self.lines, Lines::Join, Lines::Keep),
            Toggle::RemoveLines => self.lines = pick(self.lines, Lines::Remove, Lines::Keep),
        }
    }

    /// Whether this toggle currently reads as on.
    pub fn is_set(&self, toggle: Toggle) -> bool {
        match toggle {
            Toggle::Punctuation => self.punctuation,
            Toggle::Digits => self.digits,
            Toggle::RemoveSpaces => self.spaces == Whitespace::Remove,
            Toggle::NormalizeSpaces => self.spaces == Whitespace::Normalize,
            Toggle::JoinLines => self.lines == Lines::Join,
            Toggle::RemoveLines => self.lines == Lines::Remove,
        }
    }

    /// This setting with one toggle flipped, for asking whether it would matter.
    fn with(&self, toggle: Toggle) -> Self {
        let mut copy = *self;
        copy.apply(toggle);
        copy
    }
}

/// A three-valued setting flipping between `on` and `off`.
fn pick<T: PartialEq>(current: T, on: T, off: T) -> T {
    if current == on {
        off
    } else {
        on
    }
}

/// One row of the case window: a style, what it would produce, and whether that is a change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub style: Style,
    pub text: String,
    /// False when applying this would replace the selection with what is already there. The
    /// window marks such a row rather than removing it, and committing one changes nothing
    /// instead of paying for a clipboard round trip to write the same characters back.
    pub changes: bool,
}

/// Every style's result for this selection, in the declared order. Never filtered.
pub fn generate(text: &str, strips: &Strips) -> Vec<Row> {
    Style::ALL
        .into_iter()
        .map(|style| {
            let result = render(text, style, strips);
            Row {
                changes: result != text,
                style,
                text: result,
            }
        })
        .collect()
}

/// Whether flipping `toggle` would change what `style` produces for this selection.
///
/// Computed by rendering both ways and comparing. There is no table of which option can reach
/// which style, so a style added later needs no entry in one, and the awkward cases fall out on
/// their own: with whitespace removed, joining and removing line breaks agree, so whichever of
/// that pair is redundant reports false without anybody having written that down.
pub fn toggle_affects(text: &str, style: Style, strips: &Strips, toggle: Toggle) -> bool {
    render(text, style, &strips.with(toggle)) != render(text, style, strips)
}

/// Apply one style with these options.
pub fn render(text: &str, style: Style, strips: &Strips) -> String {
    let tokens = split(text);

    // The leading separator is index 0; the one following word *i* is index *i+1*. Every
    // separator survives stripping unless an option aimed at it removed it — a word emptied by
    // the digits option leaves its spaces behind, because the digits option removes digits and
    // nothing else. `hello 42 there` with digits stripped is `hello  there`, with the two
    // spaces the original had; `normalize` is how a user asks for one. Being literal here keeps
    // the options composable and keeps "what did that do?" answerable.
    let mut separators: Vec<String> = vec![resolve(&tokens.leading, strips, style)];
    let mut words: Vec<String> = Vec::new();
    for (word, following) in tokens.words {
        words.push(strip_word(&word, strips, style));
        separators.push(resolve(&following, strips, style));
    }

    // Positions are counted over the words that still have something in them, so a word emptied
    // by stripping cannot take the first-word position and leave `camelCase` capitalising what
    // should have been its lowercase head.
    let mut position = 0;
    let cased: Vec<String> = words
        .iter()
        .map(|w| {
            if w.is_empty() {
                return String::new();
            }
            let out = case_word(w, style, position);
            position += 1;
            out
        })
        .collect();

    if style.is_identifier() {
        // The style supplies the delimiter, so the original separators — leading and trailing
        // included — play no part. Emptied words are dropped rather than joined, or the
        // delimiter would double around the gap they left.
        return cased
            .iter()
            .filter(|w| !w.is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join(style.delimiter());
    }

    let mut out = String::with_capacity(text.len());
    out.push_str(&separators[0]);
    for (i, word) in cased.iter().enumerate() {
        out.push_str(word);
        if let Some(sep) = separators.get(i + 1) {
            out.push_str(sep);
        }
    }
    // Applied to the assembled text rather than to each separator, because a word emptied by
    // stripping leaves its two separators adjacent and the user sees one run, not two. Safe to
    // do here because a word can never contain whitespace: whitespace is what ends one.
    horizontal(&out, strips.spaces)
}

/// Apply the horizontal-whitespace option to assembled text.
///
/// Line breaks are left exactly as they are and end a run, so normalizing text whose lines were
/// kept does not quietly pull them together.
fn horizontal(text: &str, policy: Whitespace) -> String {
    match policy {
        Whitespace::Keep => text.to_string(),
        Whitespace::Remove => text
            .chars()
            .filter(|c| is_break(*c) || !c.is_whitespace())
            .collect(),
        Whitespace::Normalize => {
            let mut out = String::with_capacity(text.len());
            let mut in_run = false;
            for c in text.chars() {
                if is_break(c) {
                    out.push(c);
                    in_run = false;
                } else if c.is_whitespace() {
                    if !in_run {
                        out.push(' ');
                        in_run = true;
                    }
                } else {
                    out.push(c);
                    in_run = false;
                }
            }
            out
        }
    }
}

/// The selection as words and the separators around them.
struct Tokens {
    /// Whatever came before the first word.
    leading: String,
    /// Each word and the separator that follows it.
    words: Vec<(String, String)>,
}

/// Whether a character ends a word and begins a separator.
///
/// Whitespace, and the three characters identifiers are conventionally written with. A comma or
/// an exclamation mark is *not* a separator: it stays part of its word, where the punctuation
/// option can remove it and the identifier styles drop it regardless.
fn is_separator(c: char) -> bool {
    c.is_whitespace() || c == '_' || c == '-' || c == '.'
}

fn is_break(c: char) -> bool {
    c == '\n' || c == '\r'
}

/// Split into words and separators.
///
/// A lowercase letter followed by an uppercase one is a boundary with no character to show for
/// it — an empty separator — which is how an existing identifier is re-cased rather than treated
/// as one long word. A letter beside a digit is deliberately *not* a boundary: splitting there
/// turns `getUserV2` into `get_user_v_2`.
fn split(text: &str) -> Tokens {
    let chars: Vec<char> = text.chars().collect();
    let mut leading = String::new();
    let mut words: Vec<(String, String)> = Vec::new();
    let mut current = String::new();
    let mut separator = String::new();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        if is_separator(c) {
            if current.is_empty() && words.is_empty() {
                leading.push(c);
            } else if current.is_empty() {
                // Still inside the separator that follows the previous word.
                if let Some(last) = words.last_mut() {
                    last.1.push(c);
                }
            } else {
                separator.push(c);
                words.push((std::mem::take(&mut current), std::mem::take(&mut separator)));
            }
            i += 1;
            continue;
        }
        // A hump: end the word here and start the next with nothing in between.
        if !current.is_empty() {
            let previous = chars[i - 1];
            if previous.is_lowercase() && c.is_uppercase() {
                words.push((std::mem::take(&mut current), String::new()));
            }
        }
        current.push(c);
        i += 1;
    }
    if !current.is_empty() {
        words.push((current, String::new()));
    }

    Tokens { leading, words }
}

/// Take the stripped characters out of one word.
fn strip_word(word: &str, strips: &Strips, style: Style) -> String {
    word.chars()
        .filter(|c| {
            if (strips.punctuation || style.is_identifier()) && !c.is_alphanumeric() {
                return false;
            }
            if strips.digits && c.is_numeric() {
                return false;
            }
            true
        })
        .collect()
}

/// Turn one separator into what the punctuation and line options say it should be.
///
/// Horizontal whitespace is deliberately *not* handled here — see [`horizontal`], which runs
/// over the assembled text so that two separators left adjacent by an emptied word read as the
/// one run the user can see.
fn resolve(separator: &str, strips: &Strips, style: Style) -> String {
    if style.is_identifier() {
        // Never used: the style joins with its own delimiter.
        return String::new();
    }

    // Punctuation in a separator is the `_`, `-` or `.` that made the boundary. Whitespace is
    // not punctuation and survives this step.
    let stage: String = if strips.punctuation {
        separator.chars().filter(|c| c.is_whitespace()).collect()
    } else {
        separator.to_string()
    };

    match strips.lines {
        Lines::Keep => stage,
        // A carriage-return/newline pair is one break, not two, or a Windows file would
        // gain two spaces where a Unix one gains one.
        Lines::Join => stage.replace("\r\n", "\n").replace(['\n', '\r'], " "),
        Lines::Remove => stage.replace("\r\n", "\n").replace(['\n', '\r'], ""),
    }
}

/// Apply the style's casing to one word. `index` is its position in the sequence, which is what
/// makes the first word of `camelCase` and of a sentence different from the rest.
fn case_word(word: &str, style: Style, index: usize) -> String {
    match style {
        Style::AsTyped => word.to_string(),
        Style::Invert => word.chars().flat_map(invert).collect(),
        Style::Lower | Style::Snake | Style::Dot | Style::KebabLower => word.to_lowercase(),
        Style::Upper | Style::ScreamingSnake | Style::KebabUpper => word.to_uppercase(),
        Style::Pascal | Style::CapitalizedSnake | Style::Title => capitalise(word),
        Style::Camel => {
            if index == 0 {
                word.to_lowercase()
            } else {
                capitalise(word)
            }
        }
        Style::Sentence => {
            if index == 0 {
                capitalise(word)
            } else {
                word.to_lowercase()
            }
        }
    }
}

/// Swap one character's case, as an iterator because a single character does not always map to
/// a single character — German `ß` uppercases to `SS`.
fn invert(c: char) -> Box<dyn Iterator<Item = char>> {
    if c.is_lowercase() {
        Box::new(c.to_uppercase())
    } else if c.is_uppercase() {
        Box::new(c.to_lowercase())
    } else {
        Box::new(std::iter::once(c))
    }
}

/// First character up, the rest down.
///
/// Lowercasing the remainder rather than leaving it alone, so that a style meant to impose a
/// shape imposes it: `Title` on `HELLO WORLD` gives `Hello World`, and text that is already
/// shouting is not a case these styles decline to handle. Leaving the remainder untouched is
/// still reachable — that is what `As typed` is.
fn capitalise(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first
            .to_uppercase()
            .chain(chars.flat_map(char::to_lowercase))
            .collect(),
    }
}
