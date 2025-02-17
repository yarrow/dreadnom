use std::{fs, fs::File, io::Write, str, str::FromStr, sync::LazyLock};

use anyhow::{bail, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use regex::bytes::Regex;

use crate::parse::{name_copyright_body, parse};

pub fn reformat_for_obsidian(source: &Utf8PathBuf, obsidian: &Utf8PathBuf) -> Result<()> {
    const PRE_PROLOGUE: &[u8] = b"---\nobsidianUIMode: preview\n---\n\n";

    const TXT: &str = ".txt";

    // Get the names of the source files
    let article_names = gather_and_validate_visible_files_in(source, TXT)?;
    if article_names.is_empty() {
        bail!("No articles found in directory {source}");
    } else if let Some(unnumbered) =
        article_names.iter().find(|&a| number_and_name_from(a).0.is_none())
    {
        bail!(
            "All articles must start with a number, but found {unnumbered} in directory {source}"
        );
    }

    // Ensure that `obsdian` exists and contains only `.md` files (or ignored files)
    if obsidian.read_dir_utf8().is_err() {
        fs::create_dir(obsidian).with_context(|| format!("Can't create directory {obsidian}"))?;
    }
    // For `obsidian` we don't need the files, just the validation
    gather_and_validate_visible_files_in(obsidian, ".md")?;

    // Create a .md file in `obsidian` for each `.txt` file in `source`
    for txt_name in article_names {
        if txt_name.ends_with(" copy.txt") {
            // This avoids a duplicate file in Thingonomicon
            continue;
        }
        if txt_name == "00 Read Me.txt" {
            // This Laironomicon intro file doesn't have a copyright line, and we'll be supplying our own 00 README
            continue;
        }
        let source_path = source.join(&txt_name);
        let original = fs::read_to_string(&source_path)?;

        let special_case;
        let (content_name, prologue, to_be_parsed) = match urban_idea_special_case(&original) {
            Some((name, parseable)) => {
                special_case = parseable;
                (name, String::new(), &special_case[..])
            }
            None => name_copyright_body(&original)
                .with_context(|| format!("Can't understand file {source_path}"))?,
        };

        let (Some(n), fs_name) = number_and_name_from(&txt_name) else {
            bail!("This can't happen: all article_names start with a number");
        };
        let (_, content_name) = number_and_name_from(&content_name);
        let description = if n == 12 {
            // `content_name` is correct for the two `12*` files in the Thingonomicon
            // and (as it happens) for the one `12*` files in the Laironomicon
            content_name
        } else if fs_name.len() > content_name.len() {
            fs_name
        } else {
            content_name
        };

        // Currently there's only one file with a number >= 100; we choose to
        // let that one sort to the end without a number rather than use three digits.
        let output_name = if n < 100 { format!("{n:02} {description}") } else { description };

        let mut body = prologue;
        let parsed = parse(&output_name, to_be_parsed)
            .with_context(|| format!("Can't understand file {source_path}"))?;
        body.push_str(&parsed.to_string());

        let output_path = obsidian.join(&output_name).with_extension("md");
        let mut output = File::create(&output_path)?;
        output.write_all(PRE_PROLOGUE)?;
        output.write_all(body.as_bytes())?
    }

    Ok(())
}

fn gather_and_validate_visible_files_in(dir: &Utf8Path, extension: &str) -> Result<Vec<String>> {
    let entries = dir.read_dir_utf8().with_context(|| format!("Can't open directory {dir}"))?;
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.with_context(|| "Error reading directory {dir}")?;
        let path = entry.path();
        let meta = fs::metadata(path).with_context(|| "Error reading {path}")?;
        if meta.is_file() {
            let Some(name) = path.file_name() else { continue };
            if !(name.starts_with('.')) {
                let name = name.to_string();
                if !name.ends_with(extension) {
                    bail!("Files in {dir} should end in {extension} but found {name}");
                }

                names.push(name.to_string());
            }
        }
    }
    Ok(names)
}

fn urban_idea_special_case(contents: &str) -> Option<(String, String)> {
    static URBAN: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^#\s+71:? Urban.*\n#ideas\s*(1.)").unwrap());

    if let Some(urb) = URBAN.captures(contents.as_bytes()) {
        let start = urb.get(1).unwrap().start();
        return Some((
            "71 Urban Events".to_string(),
            ["\n## Ideas\n", &contents[start..]].concat(),
        ));
    }
    None
}

fn number_and_name_from(name: &str) -> (Option<u32>, String) {
    static PARTS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^(\d+)?[\s_]*(.*?)(?:.txt)?$").unwrap());
    let Some(cap) = PARTS.captures(name.as_bytes()) else {
        return (None, name.to_string());
    };
    let n = match cap.get(1) {
        Some(n_bytes) => {
            let n_str = str::from_utf8(n_bytes.as_bytes()).unwrap();
            Some(u32::from_str(n_str).unwrap())
        }
        None => None,
    };
    (n, String::from_utf8(cap[2].to_vec()).unwrap())
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn special_case_for_urban_ideas() {
        let prologue1 = "# 71 Urban\n#ideas\n";
        let prologue2 = "# 71: Urban Cities\n#ideas\n\n\n";
        let body = "1. blah blah\n 2.blah diddy blah\n";
        for prologue in [prologue1, prologue2] {
            let contents = [prologue, body].concat();
            assert_eq!(
                urban_idea_special_case(&contents).unwrap(),
                ("71 Urban Events".to_string(), ["\n## Ideas\n", body].concat())
            );
        }
    }

    #[test]
    fn number_and_name_from_splits_initial_number_from_rest() {
        let a = "12_stuff.txt";
        let b = "12 stuff";
        let c = "stuff.txt";
        assert_eq!(number_and_name_from(a), (Some(12), "stuff".to_string()));
        assert_eq!(number_and_name_from(b), (Some(12), "stuff".to_string()));
        assert_eq!(number_and_name_from(c), (None, "stuff".to_string()));
    }
}
