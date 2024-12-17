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
    clippy::items_after_statements,
    clippy::missing_errors_doc,
    clippy::semicolon_if_nothing_returned,
    clippy::struct_excessive_bools,
    clippy::let_underscore_untyped
)]
#![cfg_attr(debug_assertions, allow(dead_code, unused_imports, unused_variables))]

use std::fs::{self, File};
use std::io::Write;

use anyhow::{bail, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;

#[derive(Parser)]
struct Args {
    source: Utf8PathBuf,
    obsidian: Utf8PathBuf,
}

fn main() -> Result<()> {
    let Args { source, obsidian } = Args::parse();

    // Get the names of the source files
    let article_names = gather_and_validate_visible_files_in(&source, ".txt")?;
    if article_names.is_empty() {
        bail!("No articles found in directory {source}");
    }

    // Ensure that `obsdian` exists and contains only `.md` files (or ignored files)
    if obsidian.read_dir_utf8().is_err() {
        fs::create_dir(&obsidian).with_context(|| format!("Can't create directory {obsidian}"))?;
    }
    // For `obsidian` we don't need the files, just the validation
    gather_and_validate_visible_files_in(&obsidian, ".md")?;

    // Create a .md file in `obsidian` for each `.txt` file in `source`
    for article in article_names {
        let source_path = source.join(&article);
        let output_path = obsidian.join(&article).with_extension("md");

        let original = fs::read_to_string(&source_path)?;
        let mut output = File::create(&output_path)?;
        output.write_all(obsidianize(&original).as_bytes())?
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

                names.push(name);
            }
        }
    }
    Ok(names)
}

fn obsidianize(original: &str) -> String {
    original.to_string()
}
