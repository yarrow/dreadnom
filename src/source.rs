use std::{fs, io};

use anyhow::{Context, Result, bail};
use camino::{Utf8Path, Utf8PathBuf};
use zip::ZipArchive;

// We need `&mut self` in some methods for `DreadZipfile`:
// a `ZipArchive` has a mutable reader internally
pub(crate) trait DreadReader: Sized {
    fn new(location: &Utf8Path, extension: &str) -> Result<Self>;
    fn location(&self) -> String;
    fn extension(&self) -> String;
    fn raw_paths(&mut self) -> Result<Vec<Utf8PathBuf>>;
    fn validated_article_names(&mut self) -> Result<Vec<String>> {
        let mut validated = Vec::new();
        for path in self.raw_paths()? {
            let Some(stem) = path.file_stem() else { continue };
            if stem.starts_with('.') {
                continue;
            }
            let Some(path_extension) = path.extension() else { continue };
            if path_extension != self.extension() {
                bail!(
                    "Files in {} should end in {} but found {stem}.{path_extension}",
                    self.location(),
                    self.extension(),
                );
            }
            validated.push(stem.to_string());
        }
        Ok(validated)
    }
    fn article(&mut self, article_stem: &str) -> Result<String>;
}

pub(crate) struct DreadDirectory {
    location: Utf8PathBuf,
    extension: String,
}

impl DreadReader for DreadDirectory {
    fn new(location: &Utf8Path, extension: &str) -> Result<Self> {
        let location = location.to_owned();
        let extension = extension.to_owned();
        Ok(Self { location, extension })
    }
    fn location(&self) -> String {
        self.location.clone().into_string()
    }
    fn extension(&self) -> String {
        self.extension.to_string()
    }
    fn raw_paths(&mut self) -> Result<Vec<Utf8PathBuf>> {
        let mut relevant = Vec::new();
        let location = self.location.as_path();
        let entries =
            location.read_dir_utf8().with_context(|| format!("Can't open directory {location}"))?;
        for entry in entries {
            let entry = entry.with_context(|| "Error reading an entry in {location}")?;
            if entry.metadata()?.is_file() {
                relevant.push(entry.path().to_owned());
            }
        }
        Ok(relevant)
    }
    fn article(&mut self, article_stem: &str) -> Result<String> {
        let article_path = self.location.join(article_stem).with_extension(&self.extension);
        Ok(fs::read_to_string(&article_path)?)
    }
}

pub(crate) struct DreadZipfile {
    location: Utf8PathBuf,
    extension: String,
    archive: ZipArchive<fs::File>,
}
impl DreadReader for DreadZipfile {
    fn new(location: &Utf8Path, extension: &str) -> Result<Self> {
        let file = fs::File::open(location)?;
        let archive = ZipArchive::new(file)?;
        let location = location.to_owned();
        let extension = extension.to_owned();
        Ok(Self { location, extension, archive })
    }
    fn location(&self) -> String {
        self.location.clone().into_string()
    }
    fn extension(&self) -> String {
        self.extension.to_string()
    }
    fn raw_paths(&mut self) -> Result<Vec<Utf8PathBuf>> {
        let mut relevant = Vec::new();
        for j in 0..self.archive.len() {
            let entry = self.archive.by_index(j)?;
            if let Some(path) = entry.enclosed_name() {
                if entry.is_file() {
                    relevant.push(Utf8PathBuf::try_from(path)?);
                }
            }
        }
        Ok(relevant)
    }
    fn article(&mut self, article_stem: &str) -> Result<String> {
        let name = Utf8Path::new(article_stem).with_extension(&self.extension);
        let file = self.archive.by_name(name.as_path().as_str())?;
        Ok(io::read_to_string(file)?)
    }
}
