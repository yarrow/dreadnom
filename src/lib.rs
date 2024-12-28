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
    clippy::enum_glob_use,
    clippy::items_after_statements,
    clippy::missing_errors_doc,
    clippy::semicolon_if_nothing_returned,
    clippy::struct_excessive_bools,
    clippy::let_underscore_untyped
)]
#![cfg_attr(debug_assertions, allow(dead_code, unused_imports, unused_mut, unused_variables))]

use anyhow::{self, bail, Result};
use bstr::{BString, ByteSlice, B};
use logos::Logos;
use regex::bytes::Regex;
use std::{str, sync::LazyLock};

type LineStorage = Vec<Vec<u8>>;

pub fn obsidianize(contents: &str) -> Result<(String, String)> {
    if let Some((file_name, body)) = urban_idea_special_case(contents) {
        return Ok((file_name, body));
    }

    let (file_name, prologue, trimmed_contents) = extract_prologue(contents)?;
    let mut result = prologue;

    let body = parse(&file_name, trimmed_contents.as_bytes())?;
    result.push_str(&body.to_string());

    Ok((file_name, result))
}

fn urban_idea_special_case(contents: &str) -> Option<(String, String)> {
    static URBAN: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^#\s+71:? Urban.*\n#ideas\s*(1.)").unwrap());
    const URBAN_FIX: &[u8] = b"\n## Ideas\n";

    if let Some(urb) = URBAN.captures(contents.as_bytes()) {
        let start = urb.get(1).unwrap().start();
        return Some(("71 Urban Events".to_string(), ["## Ideas\n", &contents[start..]].concat()));
    }
    None
}

fn extract_prologue(contents: &str) -> Result<(String, String, &str)> {
    const COPYRIGHT: &str = "©";
    static HORIZONTAL_RULE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^---\s*").unwrap());
    static OGL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:Include )?OGL\s").unwrap());

    let mut contents = contents.as_bytes();
    let mut lines = contents.as_bstr().lines_with_terminator();
    let header = lines.next().unwrap();
    let mut len_taken = header.len();

    if header.starts_with(b"## 00 Read Me") {
        return Ok((
            "00 Read Me".to_string(),
            contents[len_taken..].to_str_lossy().to_string(),
            "",
        ));
    }

    let obsidian_file_name = file_name_from_header(header)?;

    let mut prologue = Vec::new();
    for line in lines.by_ref() {
        len_taken += line.len();
        if line.as_bstr().contains_str(COPYRIGHT) || OGL.is_match(line) {
            prologue.push(line.to_owned());
            break;
        }
    }
    if prologue.is_empty() {
        bail!("It doesn't contain a copyright symbol ({COPYRIGHT})");
    }

    for line in lines {
        len_taken += line.len();
        if line.as_bstr().contains_str(COPYRIGHT) {
            prologue.push(line.to_owned());
        } else if HORIZONTAL_RULE.is_match(line) {
            let mut line = line;
            if line[line.len() - 1] == b'\n' {
                line = &line[..line.len() - 1];
                len_taken -= 1;
            }
            prologue.push(b"\n".to_vec());
            prologue.push(line.to_owned());
            break;
        }
    }
    let remainder = if len_taken < contents.len() { &contents[len_taken..] } else { "".as_bytes() };
    Ok((
        obsidian_file_name,
        prologue.concat().to_str_lossy().to_string(),
        str::from_utf8(remainder)?,
    ))
}

fn file_name_from_header(header: &[u8]) -> Result<String> {
    static HEADER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^#+\s+(.*\S)\s*").unwrap());
    static THINGS_20: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^(?:20 Things #|Monstrous Lair #)(.*)").unwrap());
    static COLON: LazyLock<Regex> = LazyLock::new(|| Regex::new(r":").unwrap());

    let Some(header_caps) = HEADER.captures(&header) else {
        bail!("It doesn't start with a Markdown header");
    };
    let initial_file_name = header_caps[1].trim();
    let file_name = match THINGS_20.captures(&initial_file_name) {
        Some(caps) => caps[1].trim().to_vec(),
        None => initial_file_name.trim().to_vec(),
    };

    Ok(COLON.replace(&file_name, b"").to_str_lossy().to_string())
}

#[derive(Default, Debug, Clone, PartialEq)]
enum ThisCantHappen {
    #[default]
    UnexpectedParsingError,
}

impl std::error::Error for ThisCantHappen {}
use std::fmt;
impl fmt::Display for ThisCantHappen {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Internal error: Unexpected Parsing Error")
    }
}
#[derive(Debug, Logos, PartialEq)]
#[logos(error = ThisCantHappen)]
enum LineKind {
    #[regex(b"\n\\d+\\.[^\n]*")]
    ListItem,
    #[regex(b"\n#+ [^\n]*")]
    Header,
    #[regex(b"\n[^\n]*")]
    Vanilla,
}

fn parse(name: &str, contents: &[u8]) -> Result<BString> {
    use LineKind::*;

    if contents.is_empty() {
        return Ok(BString::new(vec![]));
    }
    if contents[0] != b'\n' {
        bail!(r"Internal error: `parse(contents)` requires `contents` to start with a newline");
    }

    let mut output = LineStorage::new();
    let (mut start, mut end) = (0, 0);
    let mut previous = Vanilla;
    let (mut link, mut slot) = (Vec::<u8>::new(), 0);

    for (kind, span) in LineKind::lexer(contents).spanned() {
        let kind = kind?;
        if kind == previous {
            end = span.end;
        } else {
            output.push(contents[start..end].to_owned());
            std::ops::Range { start, end } = span;
            match previous {
                Header | Vanilla => {}
                ListItem => {
                    output[slot] = dice_code(name, &link);
                    output.push(b"\n".to_vec());
                    output.push(link.clone());
                }
            }
            match kind {
                Vanilla => {}
                Header => link = make_link(&contents[span]),
                ListItem => {
                    slot = output.len();
                    output.push(Vec::new());
                }
            }
        }
        previous = kind;
    }
    output.push(contents[start..end].to_owned());
    if previous == ListItem {
        output[slot] = dice_code(name, &link);
        output.push(b"\n".to_vec());
        output.push(link.clone());
    }

    Ok(BString::from(output.concat()))
}

#[derive(Debug, Logos, PartialEq)]
#[logos(error = ThisCantHappen)]
enum LinkToken {
    #[regex("[\\w]+")]
    Word,
    #[regex("[^\\w]+")]
    NonWord,
}

fn make_link(header: &[u8]) -> Vec<u8> {
    let header = String::from_utf8_lossy(header);
    const SEPARATOR: &str = "-";
    use LinkToken::*;
    let mut parts = vec!["^"];
    for (token, span) in LinkToken::lexer(&header).spanned() {
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
    parts.concat().to_lowercase().into_bytes()
}

fn dice_code(name: &str, link: &[u8]) -> Vec<u8> {
    [B("\n`dice: [["), name.as_bytes(), B("#"), link, B("]]`")].concat()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = "# H\n©";

    #[test]
    fn a_minimal_content_suffices() {
        assert!(extract_prologue(MINIMAL).is_ok());
    }

    #[test]
    fn special_case_for_urban_ideas() {
        let prologue1 = "# 71 Urban\n#ideas\n";
        let prologue2 = "# 71: Urban Cities\n#ideas\n\n\n";
        let body = "1. blah blah\n 2.blah diddy blah\n";
        for prologue in [prologue1, prologue2] {
            let contents = [prologue, body].concat();
            assert_eq!(
                obsidianize(&contents).unwrap(),
                ("71 Urban Events".to_string(), ["## Ideas\n", body].concat())
            );
        }
    }

    #[test]
    fn special_case_for_00_read_me() {
        let read_me = "00 Read Me";
        let rest = "blah blah\nblah diddy blah\n";
        let contents = ["## ", read_me, "\n", rest].concat();
        assert_eq!(
            extract_prologue(&contents).unwrap(),
            (read_me.to_string(), rest.to_string(), "")
        );
    }

    #[test]
    fn content_must_contain_copyright_symbol() {
        assert!(extract_prologue("# H\ncopyright").is_err());
    }

    #[test]
    #[allow(non_snake_case)]
    fn but_OGL_instead_of_copyright_is_ok() {
        assert!(extract_prologue("# H\nOGL\nis not copyright\n----\n").is_ok());
    }

    #[test]
    fn extract_prologue_does() {
        // returns file name, prologure, and body
        let input = "# Owlbear \nThanks\n©\nfoo\n©\nbar\n----\n";
        let fname = "Owlbear".to_owned();
        let prolog = "©\n©\n\n----".to_owned();
        let body = "\n";
        assert_eq!(extract_prologue(input).unwrap(), (fname, prolog, body));
    }

    const NAME: &str = "A File Name";
    #[test]
    fn parse_requires_nonempty_content_to_begin_with_a_newline() {
        let bad_content = b"How\nnow, brown cow?\n";
        assert!(parse(NAME, bad_content).is_err());
    }

    fn p(contents: &[u8]) -> BString {
        parse(NAME, contents).unwrap()
    }

    #[test]
    fn if_entire_content_is_vanilla_then_parse_returns_it_unchanged() {
        let expected = b"\nHow\nnow, brown cow?\n";
        assert_eq!(p(expected), expected.as_bstr());
    }

    #[test]
    fn make_link_result_starts_with_newline_and_hat() {
        assert_eq!(make_link(b"").as_bstr(), b"^".as_bstr());
    }

    #[test]
    fn make_link_trims_cruft_and_lowercases() {
        assert_eq!(
            make_link(b"\n@$#$@how%^&^&%NOW-you--------COW-------").as_bstr(),
            b"^how-now-you-cow".as_bstr()
        );
    }

    #[test]
    fn dice_code_inserts_name_and_link_into_a_code_template() {
        let expected = "\n`dice: [[A#B]]`".as_bytes().as_bstr();
        assert_eq!(dice_code("A", B("B")).as_bstr(), expected);
    }

    #[test]
    fn parse_adds_dice_rolling_code_to_random_lists() {
        let input = b"\n## Random List\n1. Foo\n2. Baz";
        let expected = format!(
            "\n## Random List\n`dice: [[{NAME}#^random-list]]`\n1. Foo\n2. Baz\n^random-list"
        );
        assert_eq!(p(input), expected.as_bytes().as_bstr());
    }
}
#[cfg(test)]
mod test_file_name_from_header {
    use super::*;
    // The input must be a line with a Markdown header. The header marker (#, ##, etc) is
    // trimed, as is the '20 Things #' that sometimes follows the marker. The result is then
    // trimmed of white space.
    //

    #[test]
    fn must_be_a_markdown_header() {
        assert!(file_name_from_header(b" # Too Late").is_err());
    }

    #[test]
    fn trims_header_marker_and_whitespace() {
        assert_eq!(file_name_from_header(b"#  99 Bottles\t\n").unwrap(), "99 Bottles");
    }

    #[test]
    fn trims_20_things_prefix() {
        // Some of the Raging Swan headers begin for file n begin with '20 Things #n:'.
        // We trim the '20 Things #' and the colon.
        assert_eq!(file_name_from_header(b"# 20 Things #99: Bottles\n").unwrap(), "99 Bottles");
    }

    #[test]
    fn file_name_from_header_removes_colon_everywhere() {
        assert_eq!(file_name_from_header(b"# 88: Mottles\n").unwrap(), "88 Mottles".to_string());
    }

    #[test]
    fn markdown_can_be_header_2_etc() {
        for octo in ["#", "##", "####"] {
            let header = format!("{octo} 99 Bottles");
            assert_eq!(file_name_from_header(header.as_bytes()).unwrap(), "99 Bottles");
        }
    }
}
