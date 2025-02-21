use std::{fs, fs::File, io::Write, str, str::FromStr, sync::LazyLock};

use anyhow::{Context, Result, bail};
use camino::Utf8PathBuf;
use regex::Regex;
use serde::Serialize;
use tinytemplate::{TinyTemplate, format_unescaped};

use crate::parse::{name_copyright_body, parse};
use crate::source::{DreadDirectory, DreadReader, DreadZipfile};

pub fn reformat_for_obsidian(source: &Utf8PathBuf, obsidian: &Utf8PathBuf) -> Result<()> {
    if !source.try_exists()? {
        bail!("Source {source} does not exist")
    }
    if source.is_dir() {
        reformat(&mut DreadDirectory::new(source, "txt")?, obsidian)
    } else {
        let mut zip = DreadZipfile::new(source, "txt").with_context(|| {
            format!("Source {source} doesn't seem to be either a directory or a valid Zip archive")
        })?;
        reformat(&mut zip, obsidian)
    }
}
fn reformat(source: &mut impl DreadReader, obsidian: &Utf8PathBuf) -> Result<()> {
    let location = source.location();
    let article_names = source.validated_article_names()?;
    if article_names.is_empty() {
        bail!("No articles found in {location}");
    } else if let Some(unnumbered) =
        article_names.iter().find(|&a| number_and_title_from(a).0.is_none())
    {
        bail!("All articles must start with a number, but found {unnumbered} in {location}");
    }

    // Ensure that `obsdian` exists and contains only `.md` files (or ignored files)
    if obsidian.read_dir_utf8().is_err() {
        fs::create_dir(obsidian).with_context(|| format!("Can't create directory {obsidian}"))?;
    }
    // For `obsidian` we don't need the files, just the validation
    DreadDirectory::new(obsidian, "md")?.validated_article_names()?;

    let mut readme_info = ReadmeInfo::default();
    // Create a .md file in `obsidian` for each `.txt` file in `location`
    for external_name in article_names {
        if external_name.ends_with(" copy") {
            // This avoids a duplicate file in Thingonomicon
            continue;
        }
        let article = source.article(&external_name)?;
        if external_name == "00 Read Me" {
            // This Laironomicon intro file doesn't have a copyright line, and we'll be supplying our own Read Me file
            readme_info.save_original_readme(article);
            continue;
        }
        readme_info.update_from_article(&article);

        let special_case;
        let (content_title, prologue, to_be_parsed) = match urban_idea_special_case(&article) {
            Some((name, parseable)) => {
                special_case = parseable;
                (name, String::new(), &special_case[..])
            }
            None => name_copyright_body(&article).with_context(|| {
                format!("Can't understand article {external_name} in {location}")
            })?,
        };

        let (Some(n), external_title) = number_and_title_from(&external_name) else {
            bail!("This can't happen: all article_names start with a number");
        };
        let (_, content_title) = number_and_title_from(&content_title);
        let description = if n == 12 {
            // `content_title` is correct for the two `12*` files in the Thingonomicon
            // and (as it happens) for the one `12*` files in the Laironomicon
            content_title
        } else if external_title.len() > content_title.len() {
            external_title
        } else {
            content_title
        };

        // Currently there's only one file with a number >= 100; we choose to
        // let that one sort to the end without a number rather than use three digits.
        let output_name = if n < 100 { format!("{n:02} {description}") } else { description };

        let mut body = prologue;
        let parsed = parse(&output_name, to_be_parsed)
            .with_context(|| format!("Can't understand article {external_name} in {location}"))?;
        body.push_str(&parsed.to_string());

        write_markdown(obsidian, &output_name, &body)?;
    }

    if let Some(readme) = readme_info.readme() {
        write_markdown(obsidian, "00 - READ ME FIRST", &readme)?;
    }

    Ok(())
}

fn number_and_title_from(name: &str) -> (Option<u32>, String) {
    static PARTS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d+)?[\s_]*(.*)?$").unwrap());
    match PARTS.captures(name) {
        Some(cap) => {
            let n = cap.get(1).map(|n_str| u32::from_str(n_str.as_str()).unwrap());
            (n, cap[2].to_string())
        }
        None => (None, name.to_string()),
    }
}

fn write_markdown(obsidian: &Utf8PathBuf, output_name: &str, body: &str) -> Result<()> {
    const PRE_PROLOGUE: &[u8] = b"---\nobsidianUIMode: preview\n---\n\n";
    let output_path = obsidian.join(output_name).with_extension("md");
    let mut output = File::create(&output_path)?;
    output.write_all(PRE_PROLOGUE)?;
    output.write_all(body.as_bytes())?;
    Ok(())
}

#[derive(Default)]
struct ReadmeInfo {
    nomicon: Option<String>,
    thank_you: Option<String>,
    original_readme: Option<String>,
}
#[derive(Serialize)]
struct ReadmeContext {
    nomicon: String,
    thank_you: String,
    original_readme: String,
}
impl ReadmeInfo {
    fn save_original_readme(&mut self, original: String) {
        self.original_readme = Some(original);
    }
    fn update_from_article(&mut self, article: &str) {
        static THANKS_TO: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"(?m)^Thank you to.*?$").unwrap());
        static WHAT_NOMICON: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"(?m)^Monstrous Lair|^20 Things").unwrap());
        if self.thank_you.is_none() {
            self.thank_you = THANKS_TO.captures(article).map(|cap| cap[0].to_string());
        }
        if self.nomicon.is_none() {
            self.nomicon = WHAT_NOMICON.captures(article).map(|cap| {
                (if &cap[0] == "Monstrous Lair" { "Laironomicon" } else { "Thingonomicon" })
                    .to_string()
            });
        }
    }
    fn readme(&self) -> Option<String> {
        static TEMPLATE_TEXT: &str = include_str!("readme-template.md");
        let context = self.context()?;
        let mut template = TinyTemplate::new();
        template.add_template("readme", TEMPLATE_TEXT).unwrap();
        template.set_default_formatter(&format_unescaped);
        Some(template.render("readme", &context).unwrap())
    }
    fn context(&self) -> Option<ReadmeContext> {
        let (Some(nomicon), Some(thank_you)) = (self.nomicon.clone(), self.thank_you.clone())
        else {
            return None;
        };
        let original_readme = match &self.original_readme {
            Some(r) => ["\n\n-----\n\nHere is the original Read Me\n\n", r].concat(),
            None => String::new(),
        };
        Some(ReadmeContext { nomicon, thank_you, original_readme })
    }
}

fn urban_idea_special_case(contents: &str) -> Option<(String, String)> {
    static URBAN: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^#\s+71:? Urban.*\n#ideas\s*(1.)").unwrap());

    if let Some(urb) = URBAN.captures(contents) {
        let start = urb.get(1).unwrap().start();
        return Some((
            "71 Urban Events".to_string(),
            ["\n## Ideas\n", &contents[start..]].concat(),
        ));
    }
    None
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
    fn number_and_title_from_splits_initial_number_from_rest() {
        let a = "12_stuff";
        let b = "stuff";
        assert_eq!(number_and_title_from(a), (Some(12), "stuff".to_string()));
        assert_eq!(number_and_title_from(b), (None, "stuff".to_string()));
    }
}
