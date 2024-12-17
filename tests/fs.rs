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

use std::fs::File;
use std::io::Write;
use std::process::Command;

use camino::Utf8PathBuf;

use assert_cmd::prelude::*;
use assert_fs::{fixture::ChildPath, prelude::*, TempDir};

struct Playground {
    cmd: Command,
    tmp: TempDir,
    source: ChildPath,
    obsidian: ChildPath,
}

fn create_with_files(dir: &ChildPath, names: &Vec<&str>) {
    dir.create_dir_all().unwrap();
    for name in names {
        let mut f = File::create(dir.join(name)).unwrap();
        f.write_all(b"").unwrap();
    }
}

impl Playground {
    fn new() -> Self {
        let cmd = Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap();
        let tmp = TempDir::new().unwrap();
        let source = tmp.child("source");
        let obsidian = tmp.child("obsidian");
        Self { cmd, tmp, source, obsidian }
    }
    fn source_files(self, files: &Vec<&str>) -> Self {
        create_with_files(&self.source, files);
        self
    }
    fn obsidian_files(self, files: &Vec<&str>) -> Self {
        create_with_files(&self.obsidian, files);
        self
    }
    fn cmd(&mut self) -> &mut Command {
        self.cmd.arg(self.source.path()).arg(self.obsidian.path())
    }
    fn assert_success(mut self) -> Self {
        self.cmd().assert().success();
        self
    }
    fn assert_failure(mut self) -> Self {
        self.cmd().assert().failure();
        self
    }
    fn close(self) {
        self.tmp.close().unwrap();
    }
}

#[test]
fn source_must_exist() {
    let p = Playground::new();
    p.assert_failure().close();
}

#[test]
fn source_must_not_be_empty() {
    let p = Playground::new().source_files(&Vec::new());
    p.assert_failure().close();
}

#[test]
fn source_must_contain_only_txt_files() {
    let p = Playground::new().source_files(&vec!["foo.txt", "bar.md"]);
    p.assert_failure().close();
}

#[test]
fn obsidian_need_not_exist() {
    let p = Playground::new().source_files(&vec!["foo.txt"]);
    p.assert_success().close();
}

#[test]
fn obsidian_may_exist_and_be_empty() {
    let p = Playground::new().source_files(&vec!["foo.txt"]).obsidian_files(&vec![]);
    p.assert_success().close();
}

#[test]
fn obsidian_may_contain_dot_md_files() {
    let p = Playground::new().source_files(&vec!["a.txt"]).obsidian_files(&vec!["b.md", "c.md"]);
    p.assert_success().close();
}

#[test]
fn obsidian_may_not_contain_non_md_files() {
    let p = Playground::new()
        .source_files(&vec!["foo.txt"])
        .obsidian_files(&vec!["foo.md", "bar.md", "baz.txt"]);
    p.assert_failure().close();
}

#[test]
fn dreadnom_creates_an_obsidian_file_for_each_source_file() {
    let mut p = Playground::new()
        .source_files(&vec!["foo.txt", "bar.txt", "baz.txt"])
        .obsidian_files(&vec!["foo.md"]);
    p = p.assert_success();
    let obsidian = Utf8PathBuf::from_path_buf(p.obsidian.to_path_buf()).unwrap();
    let mut result = Vec::new();
    for entry in obsidian.read_dir_utf8().unwrap() {
        result.push(entry.unwrap().path().file_name().unwrap().to_string());
    }
    result.sort();
    assert_eq!(result, vec!["bar.md", "baz.md", "foo.md"]);
    p.close();
}