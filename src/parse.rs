#![allow(clippy::enum_glob_use)]

use anyhow::{self, Context, Result, bail};
use logos::Logos;
use regex::Regex;
use std::{error, fmt, str, sync::LazyLock};

pub(crate) fn name_copyright_body(contents: &str) -> Result<(String, String, &str)> {
    static SUBHEAD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n#+\s").unwrap());
    static COPYRIGHT_OR_OGL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bOGL\b|©").unwrap());
    const COPYRIGHT: &str = "©";

    let file_name = embedded_file_name(contents)?;

    // The first line is a title, but Obsidian uses the file name as a title
    let Some(newline) = contents.find('\n') else {
        return Ok((file_name, String::new(), ""));
    };
    let contents = &contents[newline..];

    let remainder_start = match SUBHEAD.find(contents) {
        Some(subhead) => subhead.start(),
        None => contents.len(),
    };
    let (prologue, remainder) = contents.split_at(remainder_start);

    let mut copyright = Vec::new();
    let mut lines = prologue.lines();
    for line in lines.by_ref() {
        if COPYRIGHT_OR_OGL.is_match(line) {
            // Make this line a Markdown paragraph
            copyright.push(line.to_owned());
            copyright.push("\n".to_owned());
        }
    }
    if copyright.is_empty() {
        bail!("It doesn't contain a copyright symbol ({COPYRIGHT})");
    }

    Ok((file_name, copyright.concat(), remainder))
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

pub(crate) fn parse(name: &str, contents: &str) -> Result<String> {
    if contents.is_empty() {
        return Ok(String::new());
    }
    if !contents.starts_with('\n') {
        bail!(r"Internal error: `parse(contents)` requires `contents` to start with a newline");
    }

    let mut chapter = ParsedChapter::new(name, "^START");
    let mut old_kind = LineKind::Vanilla;

    for (kind, span) in LineKind::lexer(contents).spanned() {
        let kind = kind.with_context(|| format!("Seen so far: {chapter:?}"))?;
        if old_kind != kind {
            chapter.change_kind(old_kind, kind)?
        }
        chapter.push_line(kind, &contents[span]);
        old_kind = kind;
    }
    chapter.change_kind(old_kind, LineKind::Vanilla)?;

    Ok(chapter.to_string())
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
const LIST_ITEM: &str = r"\n\d+\.\s*(.*)";
#[derive(Debug, Logos, PartialEq, Clone, Copy)]
#[logos(error = ThisCantHappen)]
enum LineKind {
    #[regex("\n\\d+\\.[^\n]*")] // This regex must track LIST_ITEM above
    ListItem,

    #[regex("\n#+ [^\n]*")]
    Header,

    #[regex("\n[^\n]*")]
    Vanilla,
}

#[derive(Debug)]
struct ParsedChapter<'a> {
    name: &'a str,
    parsed: Vec<String>,
    list: Vec<&'a str>,
    link: String,
}
impl fmt::Display for ParsedChapter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        static EXTRA_NEWLINES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n\n\n+").unwrap());

        let result = self.parsed.concat();
        write!(f, "{}", EXTRA_NEWLINES.replace_all(&result, "\n\n"))
    }
}

impl<'a> ParsedChapter<'a> {
    fn new(name: &'a str, link: &str) -> Self {
        Self { name, parsed: Vec::new(), list: Vec::new(), link: link.to_string() }
    }
    fn push_line(&mut self, kind: LineKind, line: &'a str) {
        match kind {
            LineKind::ListItem => {
                self.list.push(line);
            }
            LineKind::Header => {
                self.link = make_link(line);
                self.parsed.push(line.to_string());
            }
            LineKind::Vanilla => {
                self.parsed.push(line.to_string());
            }
        }
    }
    fn change_kind(&mut self, from: LineKind, to: LineKind) -> Result<()> {
        if to == LineKind::ListItem {
            self.push_as_paragraph(dice_code(self.name, &self.link));
        } else if from == LineKind::ListItem {
            self.parsed.push(list_to_table(&self.list)?);
            self.list.clear();
            self.push_as_paragraph(self.link.clone());
        }
        Ok(())
    }
    fn push_as_paragraph(&mut self, line: String) {
        const PILCROW: &str = "\n\n";
        self.parsed.push(PILCROW.to_string());
        self.parsed.push(line);
        self.parsed.push(PILCROW.to_string());
    }
}

fn list_to_table(items: &Vec<&str>) -> Result<String> {
    static ITEM: LazyLock<Regex> = LazyLock::new(|| Regex::new(LIST_ITEM).unwrap());
    let n = items.len();
    if n == 0 {
        bail!("Internal error: there should be at least one list item");
    }
    let mut rows = vec![format!("\n| d{n} | Item |\n| --:| -- |")];
    for item in items {
        let Some(captures) = ITEM.captures(item) else {
            bail!("Internal error: this isn't a list item: {item}")
        };
        rows.push(format!("\n| {} | {} |", rows.len(), captures[1].trim()));
    }
    Ok(rows.concat())
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
        assert!(name_copyright_body(MINIMAL).is_ok());
    }

    #[test]
    fn prologue_must_contain_copyright_symbol() {
        assert!(name_copyright_body("# H\ncopyright\n## IJK").is_err());
    }

    #[test]
    fn copyright_is_required_even_if_there_are_no_subsections() {
        let read_me = "00 Read Me";
        let rest = "\nblah diddy blah\n";
        let contents = ["## ", read_me, "\n", rest].concat();
        assert!(name_copyright_body(&contents).is_err());
    }

    #[test]
    #[allow(non_snake_case)]
    fn but_OGL_instead_of_copyright_is_ok() {
        assert!(name_copyright_body("# H\nOGL\nis not copyright\n----\n## Subhead").is_ok());
    }

    #[test]
    fn name_copyright_body_does() {
        // returns file name, prologue, and body
        let input = "# Owlbear \nThanks\n©\nfoo\n©\nbar\n## Barred Owl";
        let fname = "Owlbear".to_owned();
        let prolog = "©\n©\n".to_owned();
        let body = "\n## Barred Owl";
        assert_eq!(name_copyright_body(input).unwrap(), (fname, prolog, body));
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

    fn header(n: usize) -> String {
        format!("| d{n} | Item |\n| --:| -- |")
    }

    #[test]
    fn parse_adds_dice_rolling_code_before_and_link_after_lists() {
        let input = "\n## Random List\n1. Foo\n2. Baz";
        let head = header(2);
        let expected = format!(
            "\n## Random List¶`dice: [[{NAME}#^random-list]]`¶{head}\n| 1 | Foo |\n| 2 | Baz |¶^random-list"
        );
        let input = [input, "\nCat Dog"].concat();
        let expected = [&expected, "¶Cat Dog"].concat();
        assert_eq!(parz(&input), expected);
    }

    #[test]
    fn added_material_is_preceded_and_followed_by_paragraphs() {
        let before = ["\n## X", "\n## X\ntext"];
        let after = ["## Y", "text", ""];
        let list = "1. a\n2. b";
        let table = format!("{}\n| 1 | a |\n| 2 | b |", header(2));
        let link = "^x";
        let code = format!("`dice: [[{NAME}#{link}]]`");
        for b4 in before {
            for aft in after {
                let input = [b4, "\n", list, "\n", aft].concat();
                let expected = [b4, "¶", &code, "¶", &table, "¶", link, "¶", aft].concat();
                assert_eq!(parz(&input), expected);
            }
        }
    }

    #[test]
    fn we_add_a_link_after_a_list_that_ends_the_file_even_if_it_doesnt_end_with_a_newline() {
        let input = "\n## Subhead\n1. Foo\n2. Baz";
        let head = header(2);
        let expected = format!(
            "\n## Subhead¶`dice: [[{NAME}#^subhead]]`¶{head}\n| 1 | Foo |\n| 2 | Baz |¶^subhead¶"
        );
        assert_eq!(parz(input), expected);
    }

    #[test]
    fn list_to_table_errors_on_an_empty_list() {
        assert!(list_to_table(&Vec::new()).is_err());
    }

    #[test]
    fn list_to_table_output() {
        let input = vec!["\n1. Foo", "\n2. Bar"];
        let expected = "\n| d2 | Item |\n| --:| -- |\n| 1 | Foo |\n| 2 | Bar |";
        assert_eq!(list_to_table(&input).unwrap(), expected);
    }
    #[test]
    fn check_bad_parse_regression() {
        const WEIRD: &str = "\n\n1. T\n";
        let link = "^START";
        let code = format!("`dice: [[{NAME}#{link}]]`");
        let table = format!("{}\n| 1 | T |", header(1));
        let expected = ["¶", &code, "¶", &table, "¶", link, "¶"].concat();
        assert_eq!(parz(WEIRD), expected);
    }
}
#[cfg(test)]
mod test_embedded_file_name {
    use super::*;
    // The input must be a line with a Markdown header. The header marker (#, ##, etc) is
    // trimmed, as is the '20 Things #' that sometimes follows the marker. The result is then
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

    #[allow(non_snake_case)]
    #[test]
    fn tries_to_find_a_better_name_than_Name() {
        let contents = "# Name\nWhee!\nStuff#00: Better Name. ©";
        assert_eq!(embedded_file_name(contents).unwrap(), "Better Name");
    }
}
