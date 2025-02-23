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
#![allow(
    clippy::items_after_statements,
    clippy::missing_errors_doc,
    clippy::semicolon_if_nothing_returned,
    clippy::struct_excessive_bools,
    clippy::let_underscore_untyped
)]
#![cfg_attr(debug_assertions, allow(dead_code, unused_imports, unused_variables))]

use camino::Utf8Path;
use std::process::Command;

use assert_cmd::prelude::*;

fn compare(source: &str, output: &str, baseline: &str, kind: &str) -> Vec<String> {
    let output = Utf8Path::new(output).join(kind);
    let baseline = Utf8Path::new(baseline).join(kind);

    let manifest_dir = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
    let full_source = manifest_dir.join(source);
    let full_baseline = manifest_dir.join(&baseline);
    let full_output = manifest_dir.join(&output);
    if full_output.exists() {
        std::fs::remove_dir_all(&full_output).unwrap();
    }
    Command::cargo_bin(env!("CARGO_PKG_NAME"))
        .unwrap()
        .arg(&full_source)
        .arg(&full_output)
        .assert()
        .success();
    if dir_diff::is_different(&full_output, &full_baseline).unwrap() {
        vec![format!("Output {output} is different from {baseline}")]
    } else {
        Vec::new()
    }
}

const THING_DIR: &str = "dread_sources/thing";
const THING_ZIP: &str = "dread_sources/DT_TextFiles.zip";
const LAIR_DIR: &str = "dread_sources/lair";
const LAIR_ZIP: &str = "dread_sources/Dread_Laironomicon_Text_Archive.zip";
const BASELINE: &str = "baseline_output";
const LATEST_DIR: &str = "latest_output/dir";
const LATEST_ZIP: &str = "latest_output/zip";

const NEEDED: [&str; 7] =
    [THING_DIR, THING_ZIP, LAIR_DIR, LAIR_ZIP, BASELINE, LATEST_DIR, LATEST_ZIP];

#[ignore = "The actual data won't exist in CI"]
#[test]
fn compare_to_baseline() {
    let base = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
    let missing: Vec<_> = NEEDED
        .iter()
        .filter(|f| !base.join(f).try_exists().unwrap())
        .map(ToString::to_string)
        .collect();
    assert!(missing.is_empty(), "To test against the actual data, please provide: {missing:#?}");

    let differences = [
        compare(THING_DIR, LATEST_DIR, BASELINE, "thing"),
        compare(THING_ZIP, LATEST_ZIP, BASELINE, "thing"),
        compare(LAIR_DIR, LATEST_DIR, BASELINE, "lair"),
        compare(LAIR_ZIP, LATEST_ZIP, BASELINE, "lair"),
    ]
    .concat();

    assert!(differences.is_empty(), "Different from baseline: {differences:#?}");
}
