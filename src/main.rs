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

use anyhow::Result;

use camino::{ReadDirUtf8, Utf8DirEntry, Utf8PathBuf};
use clap::Parser;

#[derive(Parser)]
struct Args {
    original: Utf8PathBuf,
    obsidian: Utf8PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    println!("{} -> {}", args.original, args.obsidian);
    Ok(())
}
