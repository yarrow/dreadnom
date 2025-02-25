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
#![allow(clippy::doc_markdown)]
#![cfg_attr(debug_assertions, allow(dead_code, unused_imports, unused_variables))]

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::{ColorChoice, Parser, builder::styling};
use color_print::cstr;

use dreadnom::reformat_for_obsidian;

const STYLES: styling::Styles = styling::Styles::styled()
    .header(styling::AnsiColor::Green.on_default().bold())
    .usage(styling::AnsiColor::Green.on_default().bold())
    .literal(styling::AnsiColor::Blue.on_default().bold())
    .placeholder(styling::AnsiColor::Cyan.on_default());

const ABOUT: &str = "Dice rolling in Obsidian for the Dread Thingonomion/Laironomicon";
const SHORT: &str = concat!(
    cstr!("\nUse <bold,blue>dreadnom</> to adjust your purchased copy of Raging Swan's"),
    " Dread Thingonomicon/Laironomicon for digital dice rolling in Obsidian"
);
const LONG: &str = concat!(
    cstr!("\nUse <bold,blue>dreadnom</> and your purchased copy of"),
    " Raging Swan's Dread Thingonomicon or Dread Laironomicon to create",
    " a folder in your Obsidian vault where you can click each random table",
    " to choose one of the random entries.\n\n",
    cstr!(r#"See the "00 READ ME" note in the Obsidian folder <bold,blue>dreadnom</>"#),
    " creates for information about the Dice Roller plugin you'll need.",
);
#[derive(Parser)]
#[command(
    arg_required_else_help=true,
    version,
    color=ColorChoice::Auto,
    about = ABOUT,
    next_line_help = true,
    styles=STYLES,
    after_help=SHORT,
    after_long_help=LONG,
)]
struct Args {
    /// A Zip file — usually DT_TextFiles.zip for the Dread Thingonomicon
    /// or Dread_Laironomicon_Text_Archive.zip for the Dread Laironomicon.
    ///
    /// OR — a directory into which you've unzipped the contents of one of
    /// the above
    source: Utf8PathBuf,
    /// A folder inside your Obsidian vault. The folder need not currently
    /// exist. If it does, it must contain only Markdown (.md) files
    obsidian: Utf8PathBuf,
}

fn main() -> Result<()> {
    let Args { source, obsidian } = Args::parse();
    reformat_for_obsidian(&source, &obsidian)
}
