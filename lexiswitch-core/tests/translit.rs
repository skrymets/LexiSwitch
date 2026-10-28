//! Transliterating Cyrillic text into Latin letters under a ruleset.
//!
//! Pure logic, no Windows. A ruleset is a TOML text: one `[map]` of sequences where the longest
//! match wins, a `[word_start]` overlay for the start of a word, an empty value for a dropped
//! character, and an optional `fallback` for Cyrillic letters nothing covers. Capitals are never
//! written in a ruleset; they are derived from the text.

use lexiswitch_core::translit::Ruleset;

/// A small ruleset in the shape of the Ukrainian one, with only what each test needs.
const SMALL: &str = r#"
id = "small"
name = "Small"
source = "uk-UA"
target = "en-US"

[map]
"а" = "a"
"в" = "v"
"г" = "h"
"з" = "z"
"зг" = "zgh"
"и" = "y"
"і" = "i"
"ї" = "i"
"к" = "k"
"н" = "n"
"о" = "o"
"р" = "r"
"т" = "t"
"щ" = "shch"
"я" = "ia"
"ь" = ""
"'" = ""

[word_start]
"я" = "ya"
"ї" = "yi"
"#;

fn small() -> Ruleset {
    Ruleset::parse(SMALL).expect("the test ruleset is valid")
}

fn under(rules: &Ruleset, text: &str) -> String {
    rules.apply(text).text
}

// --- matching ------------------------------------------------------------------------

/// `зг` is one unit, not `з` then `г`. The longest sequence that matches wins.
#[test]
fn the_longest_sequence_wins() {
    assert_eq!(under(&small(), "розгон"), "rozghon");
}

/// The order entries are written in is not a priority. A ruleset listing `зг` before or after
/// its parts means the same thing.
#[test]
fn the_order_of_entries_does_not_matter() {
    let reordered = SMALL.replace("\"з\" = \"z\"\n\"зг\" = \"zgh\"", "\"зг\" = \"zgh\"\n\"з\" = \"z\"");
    assert_ne!(reordered, SMALL, "the replacement must have reordered something");
    let rules = Ruleset::parse(&reordered).unwrap();
    assert_eq!(under(&rules, "розгон"), "rozghon");
}

/// A prefix of a longer key that is not itself a key falls back to the shorter match.
#[test]
fn a_partial_longer_match_falls_back_to_a_shorter_one() {
    assert_eq!(under(&small(), "зо"), "zo");
    assert_eq!(under(&small(), "з"), "z");
}

/// Latin, digits, punctuation and anything else the ruleset says nothing about pass through.
#[test]
fn text_outside_the_ruleset_passes_through() {
    assert_eq!(under(&small(), "Київ 2024, Kyiv!"), "Kyiv 2024, Kyiv!");
}

/// An empty value removes the sequence.
#[test]
fn an_empty_value_drops_the_character() {
    assert_eq!(under(&small(), "ріг'ь"), "rih");
}

// --- word start ----------------------------------------------------------------------

#[test]
fn a_word_start_value_applies_at_the_start_of_the_text() {
    assert_eq!(under(&small(), "явір"), "yavir");
}

#[test]
fn the_ordinary_value_applies_inside_a_word() {
    assert_eq!(under(&small(), "ворота"), "vorota");
    assert_eq!(under(&small(), "оря"), "oria");
}

/// A space, a hyphen, a digit and punctuation all end a word.
#[test]
fn a_word_starts_after_anything_that_is_not_a_letter() {
    assert_eq!(under(&small(), "а я"), "a ya");
    assert_eq!(under(&small(), "а-я"), "a-ya");
    assert_eq!(under(&small(), "2я"), "2ya");
    assert_eq!(under(&small(), "(я)"), "(ya)");
}

/// A dropped character does not split a word: the apostrophe in `м'ясо` is part of it.
#[test]
fn a_dropped_character_does_not_start_a_word() {
    assert_eq!(under(&small(), "з'ява"), "ziava");
    assert_eq!(under(&small(), "з'їз"), "ziz");
}

/// A dropped character at the very start leaves the next letter at the start of a word.
#[test]
fn a_dropped_character_before_a_word_still_leaves_a_word_start() {
    assert_eq!(under(&small(), "'я"), "ya");
}

/// A Latin letter is a letter: Cyrillic glued to it is inside the word.
#[test]
fn a_latin_letter_before_counts_as_part_of_the_word() {
    assert_eq!(under(&small(), "xя"), "xia");
}

// --- casing --------------------------------------------------------------------------

#[test]
fn a_capitalised_word_gets_one_capital() {
    assert_eq!(under(&small(), "Щит"), "Shchyt");
    assert_eq!(under(&small(), "Згин"), "Zghyn");
}

#[test]
fn a_word_in_capitals_stays_in_capitals() {
    assert_eq!(under(&small(), "ЩИТ"), "SHCHYT");
    assert_eq!(under(&small(), "ЯР"), "YAR");
}

/// A match of two letters that are both capitals is in capitals even with no neighbour.
#[test]
fn a_two_letter_match_in_capitals_is_in_capitals() {
    assert_eq!(under(&small(), "ЗГ"), "ZGH");
    assert_eq!(under(&small(), "ЗГА"), "ZGHA");
}

/// The last letter of a word in capitals has no next letter; the previous one decides.
#[test]
fn the_last_letter_of_a_word_in_capitals_looks_back() {
    assert_eq!(under(&small(), "ОЩ"), "OSHCH");
    assert_eq!(under(&small(), "ГАЇ"), "HAI");
}

/// A single capital letter has no neighbour at all and is an initial.
#[test]
fn a_single_capital_is_title_cased() {
    assert_eq!(under(&small(), "Я"), "Ya");
    assert_eq!(under(&small(), "Щ."), "Shch.");
}

#[test]
fn lowercase_stays_lowercase() {
    assert_eq!(under(&small(), "щит"), "shchyt");
}

// --- fallback and the untransliterated count -----------------------------------------

#[test]
fn without_a_fallback_an_unknown_cyrillic_letter_passes_through() {
    let out = small().apply("ўі");
    assert_eq!(out.text, "ўi");
    assert_eq!(out.untouched, 1);
}

#[test]
fn a_fallback_replaces_an_unknown_cyrillic_letter() {
    let rules = Ruleset::parse(&SMALL.replace("target = \"en-US\"", "target = \"en-US\"\nfallback = \"?\"")).unwrap();
    let out = rules.apply("ўі");
    assert_eq!(out.text, "?i");
    assert_eq!(out.untouched, 1);
}

/// The fallback is for the source script only. Latin, digits and punctuation are never
/// replaced, so a ruleset cannot mangle the text around the Cyrillic.
#[test]
fn the_fallback_never_touches_other_scripts() {
    let rules = Ruleset::parse(&SMALL.replace("target = \"en-US\"", "target = \"en-US\"\nfallback = \"?\"")).unwrap();
    let out = rules.apply("abc 123, ß!");
    assert_eq!(out.text, "abc 123, ß!");
    assert_eq!(out.untouched, 0);
}

#[test]
fn a_fully_covered_text_leaves_nothing_untouched() {
    assert_eq!(small().apply("Київ").untouched, 0);
}

// --- parsing and validation ----------------------------------------------------------

/// Every rejection names the line, because "your ruleset is wrong" without a place to look is
/// not something a user can act on.
fn rejected(text: &str) -> (Option<usize>, String) {
    let error = Ruleset::parse(text).expect_err("this ruleset must be rejected");
    (error.line(), error.message().to_string())
}

const HEADER: &str = "id = \"t\"\nname = \"T\"\nsource = \"uk-UA\"\ntarget = \"en-US\"\n";

#[test]
fn a_file_that_does_not_parse_is_rejected_with_its_line() {
    let (line, _) = rejected(&format!("{HEADER}[map]\n\"а\" = \n"));
    assert_eq!(line, Some(6));
}

#[test]
fn an_identifier_must_be_lowercase_letters_digits_and_hyphens() {
    for id in ["", "Uk", "uk_kmu", "uk kmu", "укр"] {
        let text = HEADER.replace("id = \"t\"", &format!("id = \"{id}\""));
        let (line, message) = rejected(&text);
        assert_eq!(line, Some(1), "{id:?}");
        assert!(message.contains("id"), "{id:?}: {message}");
    }
}

#[test]
fn a_language_tag_must_be_well_formed() {
    for tag in ["", "u", "uk_UA", "uk--UA", "-uk", "uk-", "ukrainian-language", "uk-ÜA"] {
        let text = HEADER.replace("source = \"uk-UA\"", &format!("source = \"{tag}\""));
        let (line, message) = rejected(&text);
        assert_eq!(line, Some(3), "{tag:?}");
        assert!(message.contains("source"), "{tag:?}: {message}");
    }
    let text = HEADER.replace("target = \"en-US\"", "target = \"en US\"");
    assert_eq!(rejected(&text).0, Some(4));
}

#[test]
fn well_formed_tags_are_accepted() {
    for tag in ["uk", "uk-UA", "sr-Cyrl-RS", "en-US", "uk-Latn"] {
        let text = HEADER.replace("source = \"uk-UA\"", &format!("source = \"{tag}\""));
        assert!(Ruleset::parse(&text).is_ok(), "{tag:?}");
    }
}

#[test]
fn an_empty_sequence_is_rejected() {
    let (line, _) = rejected(&format!("{HEADER}[map]\n\"а\" = \"a\"\n\"\" = \"x\"\n"));
    assert_eq!(line, Some(7));
}

/// Capitals are derived. A capital in a key would look like it does something and never match.
#[test]
fn a_sequence_with_a_capital_is_rejected() {
    let (line, message) = rejected(&format!("{HEADER}[map]\n\"а\" = \"a\"\n\"Щ\" = \"Shch\"\n"));
    assert_eq!(line, Some(7));
    assert!(message.contains("lowercase"), "{message}");

    let (line, _) = rejected(&format!("{HEADER}[word_start]\n\"Я\" = \"Ya\"\n"));
    assert_eq!(line, Some(6));
}

#[test]
fn an_unknown_key_is_rejected() {
    let (line, message) = rejected(&format!("{HEADER}direction = \"both\"\n"));
    assert_eq!(line, Some(5));
    assert!(message.contains("direction"), "{message}");
}

/// A misspelled table would otherwise be silently ignored, and the ruleset would look broken.
#[test]
fn an_unknown_table_is_rejected() {
    let (line, message) = rejected(&format!("{HEADER}[wordstart]\n\"я\" = \"ya\"\n"));
    assert_eq!(line, Some(5));
    assert!(message.contains("wordstart"), "{message}");
}

#[test]
fn a_value_must_be_text() {
    let (line, _) = rejected(&format!("{HEADER}[map]\n\"а\" = 1\n"));
    assert_eq!(line, Some(6));
}

#[test]
fn a_missing_name_is_rejected() {
    let text = HEADER.replace("name = \"T\"\n", "");
    assert!(Ruleset::parse(&text).is_err());
}

// --- the built-in rulesets -----------------------------------------------------------

fn builtin(id: &str) -> Ruleset {
    lexiswitch_core::translit::builtins()
        .into_iter()
        .find(|r| r.id() == id)
        .unwrap_or_else(|| panic!("no built-in ruleset {id:?}"))
}

#[test]
fn the_builtins_are_ukrainian_then_russian() {
    let ids: Vec<String> = lexiswitch_core::translit::builtins()
        .iter()
        .map(|r| r.id().to_string())
        .collect();
    assert_eq!(ids, ["uk-kmu55", "ru-icao"]);
}

/// Every example in the table of Cabinet of Ministers of Ukraine Resolution No. 55
/// (27 January 2010), letter by letter, plus its note on `зг`.
#[test]
fn the_ukrainian_builtin_reproduces_resolution_55() {
    let uk = builtin("uk-kmu55");
    let table = [
        ("Алушта", "Alushta"), ("Андрій", "Andrii"),
        ("Борщагівка", "Borshchahivka"), ("Борисенко", "Borysenko"),
        ("Вінниця", "Vinnytsia"), ("Володимир", "Volodymyr"),
        ("Гадяч", "Hadiach"), ("Богдан", "Bohdan"), ("Згурський", "Zghurskyi"),
        ("Ґалаґан", "Galagan"), ("Ґорґани", "Gorgany"),
        ("Донецьк", "Donetsk"), ("Дмитро", "Dmytro"),
        ("Рівне", "Rivne"), ("Олег", "Oleh"), ("Есмань", "Esman"),
        ("Єнакієве", "Yenakiieve"), ("Гаєвич", "Haievych"), ("Короп'є", "Koropie"),
        ("Житомир", "Zhytomyr"), ("Жанна", "Zhanna"), ("Жежелів", "Zhezheliv"),
        ("Закарпаття", "Zakarpattia"), ("Казимирчук", "Kazymyrchuk"),
        ("Медвин", "Medvyn"), ("Михайленко", "Mykhailenko"),
        ("Іванків", "Ivankiv"), ("Іващенко", "Ivashchenko"),
        ("Їжакевич", "Yizhakevych"), ("Кадиївка", "Kadyivka"), ("Мар'їне", "Marine"),
        ("Йосипівка", "Yosypivka"), ("Стрий", "Stryi"), ("Олексій", "Oleksii"),
        ("Київ", "Kyiv"), ("Коваленко", "Kovalenko"),
        ("Лебедин", "Lebedyn"), ("Леонід", "Leonid"),
        ("Миколаїв", "Mykolaiv"), ("Маринич", "Marynych"),
        ("Ніжин", "Nizhyn"), ("Наталія", "Nataliia"),
        ("Одеса", "Odesa"), ("Онищенко", "Onyshchenko"),
        ("Полтава", "Poltava"), ("Петро", "Petro"),
        ("Решетилівка", "Reshetylivka"), ("Рибчинський", "Rybchynskyi"),
        ("Суми", "Sumy"), ("Соломія", "Solomiia"),
        ("Тернопіль", "Ternopil"), ("Троць", "Trots"),
        ("Ужгород", "Uzhhorod"), ("Уляна", "Uliana"),
        ("Фастів", "Fastiv"), ("Філіпчук", "Filipchuk"),
        ("Харків", "Kharkiv"), ("Христина", "Khrystyna"),
        ("Біла Церква", "Bila Tserkva"), ("Стеценко", "Stetsenko"),
        ("Чернівці", "Chernivtsi"), ("Шевченко", "Shevchenko"),
        ("Шостка", "Shostka"), ("Кишеньки", "Kyshenky"),
        ("Щербухи", "Shcherbukhy"), ("Гоща", "Hoshcha"), ("Гаращенко", "Harashchenko"),
        ("Юрій", "Yurii"), ("Корюківка", "Koriukivka"),
        ("Яготин", "Yahotyn"), ("Ярошенко", "Yaroshenko"), ("Костянтин", "Kostiantyn"),
        ("Знам'янка", "Znamianka"), ("Феодосія", "Feodosiia"),
        ("Згорани", "Zghorany"), ("Розгон", "Rozghon"),
    ];
    for (cyrillic, latin) in table {
        assert_eq!(uk.apply(cyrillic).text, latin, "{cyrillic}");
    }
}

/// The apostrophe is dropped in every form a Ukrainian keyboard or text produces it.
#[test]
fn the_ukrainian_builtin_drops_every_apostrophe() {
    let uk = builtin("uk-kmu55");
    for apostrophe in ['\'', '\u{2019}', '\u{02BC}'] {
        assert_eq!(uk.apply(&format!("Знам{apostrophe}янка")).text, "Znamianka", "{apostrophe:?}");
    }
}

#[test]
fn the_ukrainian_builtin_keeps_capitals() {
    let uk = builtin("uk-kmu55");
    assert_eq!(uk.apply("Щедрий Знам'янка ЩУКА").text, "Shchedryi Znamianka SHCHUKA");
    assert_eq!(uk.apply("ІВАНО-ФРАНКІВСЬК").text, "IVANO-FRANKIVSK");
    assert_eq!(uk.apply("Івано-Франківськ").text, "Ivano-Frankivsk");
}

/// ICAO Doc 9303, Part 3: the Cyrillic table as russian uses it.
#[test]
fn the_russian_builtin_follows_icao_9303() {
    let ru = builtin("ru-icao");
    let alphabet = [
        ('а', "a"), ('б', "b"), ('в', "v"), ('г', "g"), ('д', "d"), ('е', "e"), ('ё', "e"),
        ('ж', "zh"), ('з', "z"), ('и', "i"), ('й', "i"), ('к', "k"), ('л', "l"), ('м', "m"),
        ('н', "n"), ('о', "o"), ('п', "p"), ('р', "r"), ('с', "s"), ('т', "t"), ('у', "u"),
        ('ф', "f"), ('х', "kh"), ('ц', "ts"), ('ч', "ch"), ('ш', "sh"), ('щ', "shch"),
        ('ъ', "ie"), ('ы', "y"), ('ь', ""), ('э', "e"), ('ю', "iu"), ('я', "ia"),
    ];
    for (letter, latin) in alphabet {
        assert_eq!(ru.apply(&letter.to_string()).text, latin, "{letter}");
    }
    let names = [
        ("Щукин", "Shchukin"), ("Подъячев", "Podieiachev"), ("Ёлкин", "Elkin"),
        ("Юрьев", "Iurev"), ("Ульяна", "Uliana"), ("Екатеринбург", "Ekaterinburg"),
        ("Высоцкий", "Vysotskii"), ("Объект", "Obieekt"),
    ];
    for (cyrillic, latin) in names {
        assert_eq!(ru.apply(cyrillic).text, latin, "{cyrillic}");
    }
}

/// Every built-in covers its whole alphabet, so nothing is left untransliterated.
#[test]
fn the_builtins_cover_their_alphabets() {
    let uk = builtin("uk-kmu55");
    let ukrainian = "абвгґдеєжзиіїйклмнопрстуфхцчшщьюя";
    assert_eq!(uk.apply(ukrainian).untouched, 0);
    assert_eq!(uk.apply(&ukrainian.to_uppercase()).untouched, 0);

    let ru = builtin("ru-icao");
    let russian = "абвгдеёжзийклмнопрстуфхцчшщъыьэюя";
    assert_eq!(ru.apply(russian).untouched, 0);
    assert_eq!(ru.apply(&russian.to_uppercase()).untouched, 0);
}

// --- the set of rulesets offered -----------------------------------------------------

use lexiswitch_core::translit::Rulesets;

fn user(id: &str, name: &str) -> String {
    format!("id = \"{id}\"\nname = \"{name}\"\nsource = \"uk-UA\"\ntarget = \"en-US\"\n[map]\n\"а\" = \"a\"\n")
}

fn files(list: &[(&str, String)]) -> Vec<(String, Result<String, String>)> {
    list.iter().map(|(f, t)| (f.to_string(), Ok(t.clone()))).collect()
}

fn ids(set: &Rulesets) -> Vec<String> {
    set.iter().map(|r| r.id().to_string()).collect()
}

#[test]
fn with_no_files_only_the_builtins_are_offered() {
    let (set, failures) = Rulesets::build(&[], &Rulesets::builtin());
    assert_eq!(ids(&set), ["uk-kmu55", "ru-icao"]);
    assert!(failures.is_empty());
}

#[test]
fn user_rulesets_follow_the_builtins_ordered_by_name() {
    let (set, _) = Rulesets::build(
        &files(&[("a.toml", user("zz", "Zulu")), ("b.toml", user("aa", "Belarusian")), ("c.toml", user("mm", "Alpha"))]),
        &Rulesets::builtin(),
    );
    assert_eq!(ids(&set), ["uk-kmu55", "ru-icao", "mm", "aa", "zz"]);
}

#[test]
fn a_user_ruleset_replaces_the_builtin_with_its_id_in_its_place() {
    let (set, _) = Rulesets::build(&files(&[("mine.toml", user("uk-kmu55", "Mine"))]), &Rulesets::builtin());
    assert_eq!(ids(&set), ["uk-kmu55", "ru-icao"]);
    assert_eq!(set.iter().next().unwrap().name(), "Mine");
}

/// Files are read in name order, so which of two clashing files wins does not depend on the
/// order the folder happened to list them in.
#[test]
fn of_two_files_with_one_id_the_later_by_file_name_is_rejected() {
    let (set, failures) = Rulesets::build(
        &files(&[("b.toml", user("same", "B")), ("a.toml", user("same", "A"))]),
        &Rulesets::builtin(),
    );
    assert_eq!(set.iter().find(|r| r.id() == "same").unwrap().name(), "A");
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].file(), "b.toml");
    assert!(failures[0].to_string().contains("a.toml"), "{}", failures[0]);
}

#[test]
fn a_broken_file_is_reported_and_the_rest_still_load() {
    let (set, failures) = Rulesets::build(
        &files(&[("bad.toml", "id = \"bad\"\n[map\n".to_string()), ("good.toml", user("good", "Good"))]),
        &Rulesets::builtin(),
    );
    assert_eq!(ids(&set), ["uk-kmu55", "ru-icao", "good"]);
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].file(), "bad.toml");
    assert_eq!(failures[0].error().line(), Some(2));
    assert!(failures[0].to_string().starts_with("bad.toml: line 2:"), "{}", failures[0]);
}

/// A file that loaded once and is then saved with an error keeps working as it was, until the
/// error is fixed.
#[test]
fn a_file_that_breaks_keeps_its_last_good_version() {
    let (before, _) = Rulesets::build(&files(&[("mine.toml", user("mine", "Mine"))]), &Rulesets::builtin());
    let (after, failures) = Rulesets::build(&files(&[("mine.toml", "id = \"mine\"\n[map\n".to_string())]), &before);
    assert_eq!(ids(&after), ["uk-kmu55", "ru-icao", "mine"]);
    assert_eq!(failures.len(), 1);
}

/// A file that is removed takes its ruleset with it, even one that was kept from before.
#[test]
fn a_removed_file_is_no_longer_offered() {
    let (before, _) = Rulesets::build(&files(&[("mine.toml", user("mine", "Mine"))]), &Rulesets::builtin());
    let (after, _) = Rulesets::build(&[], &before);
    assert_eq!(ids(&after), ["uk-kmu55", "ru-icao"]);
}

/// A file that cannot be read — locked mid-save, say — is reported and keeps its last good
/// version, exactly as one that does not parse.
#[test]
fn an_unreadable_file_keeps_its_last_good_version() {
    let (before, _) = Rulesets::build(&files(&[("mine.toml", user("mine", "Mine"))]), &Rulesets::builtin());
    let unreadable = vec![("mine.toml".to_string(), Err("access denied".to_string()))];
    let (after, failures) = Rulesets::build(&unreadable, &before);
    assert_eq!(ids(&after), ["uk-kmu55", "ru-icao", "mine"]);
    assert_eq!(failures.len(), 1);
    assert!(failures[0].to_string().contains("access denied"), "{}", failures[0]);
}

/// The example shipped in the release zip is the built-in Ukrainian ruleset written out, and
/// says so. If the two drift apart, the example documents a scheme the program does not have.
#[test]
fn the_shipped_example_is_exactly_the_ukrainian_builtin() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../packaging/transliteration.example.toml");
    let text = std::fs::read_to_string(path).expect("packaging/transliteration.example.toml");
    let example = Ruleset::parse(&text).expect("the shipped example is a valid ruleset");
    assert_eq!(example, builtin("uk-kmu55"));
}
