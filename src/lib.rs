#![deny(
    warnings,
    clippy::all,
    clippy::cargo,
    clippy::pedantic,
    trivial_casts,
    trivial_numeric_casts,
    unused_extern_crates,
    unused_import_braces,
    unused_qualifications,
    unused_must_use
)]
#![allow(clippy::cargo)] // FIXME
#![allow(
    clippy::bool_to_int_with_if,
    clippy::comparison_to_empty,
    clippy::enum_glob_use,
    clippy::items_after_statements,
    clippy::missing_errors_doc,
    clippy::semicolon_if_nothing_returned,
    clippy::struct_excessive_bools,
    clippy::let_underscore_untyped
)]
#![cfg_attr(debug_assertions, allow(dead_code, unused_imports, unused_mut, unused_variables))]

use anyhow::{self, bail, Context, Result};
use logos::Logos;
use regex::Regex;
use std::{error, fmt, ops::Range, str, sync::LazyLock};

pub fn subdivide(contents: &str) -> Result<(String, String, &str)> {
    static SUBHEAD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n#+\s").unwrap());
    const COPYRIGHT: &str = "©";
    static OGL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:Include )?OGL\b").unwrap());

    let file_name = embedded_file_name(contents)?;

    let Some(newline) = contents.find('\n') else {
        return Ok((file_name, String::new(), ""));
    };

    let mut proem = &contents[newline..];
    let remainder;
    if let Some(sub) = SUBHEAD.find(proem) {
        remainder = &proem[sub.start()..];
        let pro_start = if sub.start() == 0 { 0 } else { 1 }; // Skip the leading '\n'
        proem = &proem[pro_start..sub.start()];
    } else {
        proem = &proem[1..];
        remainder = "";
    }

    let mut prologue = Vec::new();
    if remainder == "" {
        prologue.push(proem.to_owned());
    } else {
        let mut lines = proem.lines();
        for line in lines.by_ref() {
            if line.contains(COPYRIGHT) || OGL.is_match(line) {
                prologue.push(line.to_owned());
                prologue.push("\n".to_owned());
            }
        }
        if prologue.is_empty() {
            bail!("It doesn't contain a copyright symbol ({COPYRIGHT})");
        }
    }

    Ok((file_name, prologue.concat(), remainder))
}

fn embedded_file_name(contents: &str) -> Result<String> {
    static HEADER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^#+\s+(.*\S)\s*").unwrap());
    static THINGS_20: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^(?:20 Things #|Monstrous Lair #)(.*)").unwrap());
    static COLON: LazyLock<Regex> = LazyLock::new(|| Regex::new(r":").unwrap());

    let Some(header_caps) = HEADER.captures(contents) else {
        bail!("It doesn't start with a Markdown header");
    };
    let initial_file_name = header_caps[1].trim();
    let mut file_name = match THINGS_20.captures(initial_file_name) {
        Some(caps) => caps[1].trim().to_string(),
        None => initial_file_name.trim().to_string(),
    };
    if &file_name == "Name" {
        static FROM_COPYRIGHT_LINE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"\n[^#]+#\d\d:\s*([^.]+)\.\s*©").unwrap());
        if let Some(found) = FROM_COPYRIGHT_LINE.captures(contents) {
            file_name = found[1].to_string();
        }
    }

    Ok(COLON.replace(&file_name, "").to_string())
}

#[derive(Default, Debug, Clone, PartialEq)]
enum ThisCantHappen {
    #[default]
    UnexpectedParsingError,
}

impl error::Error for ThisCantHappen {}
impl fmt::Display for ThisCantHappen {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Internal error: Unexpected Parsing Error")
    }
}
#[derive(Debug, Logos, PartialEq)]
#[logos(error = ThisCantHappen)]
enum LineKind {
    #[regex("\n\\d+\\.[^\n]*")]
    ListItem,

    #[regex("\n#+ [^\n]*")]
    Header,

    #[regex("\n[^\n]*")]
    Vanilla,
}

struct ParseParts<'a> {
    name: &'a str,
    parts: Vec<String>,
    link: String,
}

impl<'a> ParseParts<'a> {
    fn new(name: &'a str, link: &str) -> Self {
        Self { name, parts: Vec::new(), link: link.to_string() }
    }
    fn push(&mut self, text: &str) {
        self.parts.push(text.to_string());
    }
    fn push_with_paragraph(&mut self, text: String) {
        const PARAGRAPH: &str = "\n\n";
        self.push(PARAGRAPH);
        self.parts.push(text);
        self.push(PARAGRAPH);
    }
    fn set_link(&mut self, link_text: &str) {
        self.link = make_link(link_text);
    }
    fn push_link(&mut self) {
        self.push_with_paragraph(self.link.clone());
    }
    fn push_dice_code(&mut self) {
        self.push_with_paragraph(dice_code(self.name, &self.link));
    }
    fn concat(&self) -> String {
        self.parts.concat()
    }
}

pub fn parse(name: &str, contents: &str) -> Result<String> {
    use LineKind::*;

    if contents.is_empty() {
        return Ok(String::new());
    }
    if !contents.starts_with('\n') {
        bail!(r"Internal error: `parse(contents)` requires `contents` to start with a newline");
    }

    let mut parts = ParseParts::new(name, "^START");
    let (mut start, mut end) = (0, 0);
    let mut previous = Vanilla;

    for (kind, span) in LineKind::lexer(contents).spanned() {
        let kind = kind.with_context(|| format!("Seen so far: {:?}", parts.concat()))?;
        if kind == previous {
            end = span.end;
        } else {
            parts.push(&contents[start..end]);
            if previous == ListItem {
                parts.push_link();
            }
            Range { start, end } = span;
            match kind {
                ListItem => parts.push_dice_code(),
                Header => parts.set_link(&contents[span]),
                Vanilla => {}
            }
        }
        previous = kind;
    }
    parts.push(&contents[start..end]);
    if previous == ListItem {
        parts.push_link();
    }

    static EXTRA_NEWLINES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n\n\n+").unwrap());
    Ok(EXTRA_NEWLINES.replace_all(&parts.concat(), "\n\n").to_string())
}

#[derive(Debug, Logos, PartialEq)]
#[logos(error = ThisCantHappen)]
enum LinkToken {
    #[regex("[\\w]+")]
    Word,
    #[regex("[^\\w]+")]
    NonWord,
}

fn make_link(header: &str) -> String {
    const SEPARATOR: &str = "-";
    use LinkToken::*;
    let mut parts = vec!["^"];
    for (token, span) in LinkToken::lexer(header).spanned() {
        parts.push(if token.unwrap() == Word { &header[span] } else { "-" });
    }
    if parts.len() >= 2 && parts[1] == SEPARATOR {
        parts[1] = "";
    }
    if let Some(last) = parts.last() {
        if *last == SEPARATOR {
            parts.pop();
        }
    }
    parts.concat().to_lowercase()
}

fn dice_code(name: &str, link: &str) -> String {
    ["\n`dice: [[", name, "#", link, "]]`\n"].concat()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = "# H\n©";

    #[test]
    fn a_minimal_content_suffices() {
        assert!(subdivide(MINIMAL).is_ok());
    }

    #[test]
    fn prologue_must_contain_copyright_symbol() {
        assert!(subdivide("# H\ncopyright\n## IJK").is_err());
    }

    #[test]
    fn but_if_there_are_no_subsections_then_copyright_isnt_required() {
        let read_me = "00 Read Me";
        let rest = "\nblah diddy blah\n";
        let contents = ["## ", read_me, "\n", rest].concat();
        assert_eq!(subdivide(&contents).unwrap(), (read_me.to_string(), rest.to_string(), ""));
    }

    #[test]
    #[allow(non_snake_case)]
    fn but_OGL_instead_of_copyright_is_ok() {
        assert!(subdivide("# H\nOGL\nis not copyright\n----\n## Subhead").is_ok());
    }

    #[test]
    fn subdivide_does() {
        // returns file name, prologue, and body
        let input = "# Owlbear \nThanks\n©\nfoo\n©\nbar\n## Barred Owl";
        let fname = "Owlbear".to_owned();
        let prolog = "©\n©\n".to_owned();
        let body = "\n## Barred Owl";
        assert_eq!(subdivide(input).unwrap(), (fname, prolog, body));
    }

    #[test]
    fn make_link_result_starts_with_newline_and_hat() {
        assert_eq!(make_link(""), "^");
    }

    #[test]
    fn make_link_trims_cruft_and_lowercases() {
        assert_eq!(make_link("\n@$#$@how%^&^&%NOW-you--------COW-------"), "^how-now-you-cow");
    }

    #[test]
    fn dice_code_inserts_name_and_link_into_a_code_template() {
        let expected = "\n`dice: [[A#B]]`\n";
        assert_eq!(dice_code("A", "B"), expected);
    }

    const NAME: &str = "A File Name";
    #[test]
    fn parse_requires_nonempty_content_to_begin_with_a_newline() {
        let bad_content = "How\nnow, brown cow?\n";
        assert!(parse(NAME, bad_content).is_err());
    }

    fn parz(contents: &str) -> String {
        static PARAGRAPH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n\n+").unwrap());
        let parsed = parse(NAME, contents).unwrap();
        PARAGRAPH.replace_all(&parsed, "¶").to_string()
    }

    #[test]
    fn if_entire_content_is_vanilla_then_parse_returns_it_unchanged() {
        let expected = "\nHow\nnow, brown cow?\n";
        assert_eq!(parz(expected), expected);
    }

    #[test]
    fn heading_followed_by_vanilla_does_not_introduce_a_paragraph() {
        let expected = "\n## Head\nVanilla";
        assert_eq!(parz(expected), expected);
    }

    #[test]
    fn parse_adds_dice_rolling_code_before_and_link_after_lists() {
        let input = "\n## Random List\n1. Foo\n2. Baz";
        let expected =
            format!("\n## Random List¶`dice: [[{NAME}#^random-list]]`¶1. Foo\n2. Baz¶^random-list");
        let input = [input, "\nCat Dog"].concat();
        let expected = [&expected, "¶Cat Dog"].concat();
        assert_eq!(parz(&input), expected);
    }

    #[test]
    fn added_material_is_preceeded_and_followed_by_paragraphs() {
        let before = ["\n## X", "\n## X\ntext"];
        let after = ["## Y", "text", ""];
        let list = "1. a\n2. b";
        let link = "^x";
        let code = format!("`dice: [[{NAME}#{link}]]`");
        for b4 in before {
            for aft in after {
                let input = [b4, "\n", list, "\n", aft].concat();
                let expected = [b4, "¶", &code, "¶", list, "¶", link, "¶", aft].concat();
                assert_eq!(parz(&input), expected);
            }
        }
    }

    #[test]
    fn we_add_a_link_after_a_list_that_ends_the_file_even_if_it_doesnt_end_with_a_newline() {
        let input = "\n## Subhead\n1. Foo\n2. Baz";
        let expected = format!("\n## Subhead¶`dice: [[{NAME}#^subhead]]`¶1. Foo\n2. Baz¶^subhead¶");
        assert_eq!(parz(input), expected);
    }

    #[test]
    fn check_bad_parse_regression() {
        const WEIRD: &str = "\n\n1. T\n";
        let link = "^START";
        let code = format!("`dice: [[{NAME}#{link}]]`");
        let expected = ["¶", &code, "¶", "1. T", "¶", link, "¶"].concat();
        assert_eq!(parz(WEIRD), expected);
    }
}
#[cfg(test)]
#[allow(non_snake_case)]
mod test_embedded_file_name {
    use super::*;
    // The input must be a line with a Markdown header. The header marker (#, ##, etc) is
    // trimed, as is the '20 Things #' that sometimes follows the marker. The result is then
    // trimmed of white space.
    //

    #[test]
    fn must_be_a_markdown_header() {
        assert!(embedded_file_name(" # Too Late").is_err());
    }

    #[test]
    fn trims_header_marker_and_whitespace() {
        assert_eq!(embedded_file_name("#  99 Bottles\t\n").unwrap(), "99 Bottles");
    }

    #[test]
    fn trims_20_things_prefix() {
        // Some of the Raging Swan headers begin for file n begin with '20 Things #n:'.
        // We trim the '20 Things #' and the colon.
        assert_eq!(embedded_file_name("# 20 Things #99: Bottles\n").unwrap(), "99 Bottles");
    }

    #[test]
    fn embedded_file_name_removes_colon_everywhere() {
        assert_eq!(embedded_file_name("# 88: Mottles\n").unwrap(), "88 Mottles".to_string());
    }

    #[test]
    fn markdown_can_be_header_2_etc() {
        for octo in ["#", "##", "####"] {
            let header = format!("{octo} 99 Bottles");
            assert_eq!(embedded_file_name(&header).unwrap(), "99 Bottles");
        }
    }

    #[test]
    fn tries_to_find_a_better_name_than_Name() {
        let contents = "# Name\nWhee!\nStuff#00: Better Name. ©";
        assert_eq!(embedded_file_name(contents).unwrap(), "Better Name");
    }
}
