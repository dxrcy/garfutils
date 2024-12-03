use crate::random;

use std::fs::{self, DirEntry, File};
use std::io::{self, BufRead as _, BufReader, Read, Write as _};
use std::path::Path;
use std::thread;
use std::time::Duration;

use anyhow::{bail, Context as _, Result};
use chrono::NaiveDate;
use rand::Rng as _;

pub fn discard_read_line(reader: &mut impl Read) {
    let mut reader = BufReader::new(reader);
    loop {
        let mut buffer = [0];
        reader
            .read_exact(&mut buffer)
            .expect("failed to read stdin");
        if buffer[0] == b'\n' {
            return;
        }
    }
}

pub fn get_random_directory_entry<F>(
    dir: impl AsRef<Path>,
    predicate: F,
) -> Result<Option<DirEntry>>
where
    F: FnMut(&DirEntry) -> bool,
{
    let entries = read_dir(&dir)?.flatten().filter(predicate);
    let mut entries = sort_dir_entries(entries.collect());

    if entries.len() == 0 {
        return Ok(None);
    }
    let index = random::with_rng(|rng| rng.gen_range(0..entries.len()));
    let entry = entries.swap_remove(index); // Get owned element in O(1) time
    Ok(Some(entry))
}

/// Wrapper for `fs::read_dir` which provides context for some errors
pub fn read_dir(dir: impl AsRef<Path>) -> Result<impl Iterator<Item = Result<DirEntry>>> {
    let entries =
        fs::read_dir(&dir).with_context(|| format!("Reading directory {:?}", dir.as_ref()))?;
    let entries = entries.map(|entry| entry.with_context(|| "Reading directory entry"));
    Ok(entries)
}

/// Discards any `Err` entries
pub fn sort_dir_entries(mut entries: Vec<DirEntry>) -> Vec<DirEntry> {
    entries.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    entries
}

pub fn append_date(path: impl AsRef<Path>, date: NaiveDate) -> io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{}", date)?;
    Ok(())
}

pub fn get_date_from_path(path: impl AsRef<Path>) -> Result<Option<NaiveDate>> {
    let path = path.as_ref();
    let date_str = path
        .file_stem()
        .with_context(|| "Invalid file name")?
        .to_string_lossy();
    let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d");
    Ok(date.ok())
}

pub fn read_last_line_as_date(file: File) -> Result<NaiveDate> {
    let mut reader = BufReader::new(file);
    let mut date: Option<NaiveDate> = None;

    loop {
        let mut new_line = String::new();
        let bytes_read = reader.read_line(&mut new_line)?;
        if bytes_read == 0 {
            match date {
                Some(date) => return Ok(date),
                None => bail!("File is empty"),
            }
        }
        if !new_line.trim().is_empty() {
            match NaiveDate::parse_from_str(new_line.trim(), "%Y-%m-%d") {
                Ok(new_date) => date = Some(new_date),
                Err(error) => bail!("File contains invalid date: {}", error),
            }
        }
    }
}

pub fn find_child<F>(dir: impl AsRef<Path>, predicate: F) -> Result<Option<String>>
where
    F: Fn(&Path) -> Result<bool>,
{
    let entries = sort_dir_entries(read_dir(&dir)?.flatten().collect());
    for entry in entries {
        let path = entry.path();

        if !predicate(&path)? {
            continue;
        }

        let file_name = path
            .file_name()
            .with_context(|| "Invalid file name")?
            .to_string_lossy()
            .to_string();

        return Ok(Some(file_name));
    }
    Ok(None)
}

pub fn file_matches_string(file_path: impl AsRef<Path>, target: &str) -> io::Result<bool> {
    let file = fs::OpenOptions::new().read(true).open(file_path)?;

    let mut file_bytes = BufReader::new(file).bytes();
    let mut target_bytes = target.bytes();

    // Check for any mismatched bytes
    let zipped = (&mut file_bytes).zip(&mut target_bytes);
    for (fb, tb) in zipped {
        if fb? != tb {
            return Ok(false);
        }
    }

    // If either iterator is unexhausted, then lengths mismatch
    let lengths_match = file_bytes.next().is_none() && target_bytes.next().is_none();
    Ok(lengths_match)
}

pub fn file_contains_line(file: File, needle: &str) -> io::Result<bool> {
    let reader = io::BufReader::new(file);
    for line in reader.lines() {
        let line = line?;
        if line.trim() == needle {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn wait_for_file(path: impl AsRef<Path>) {
    const WAIT_DELAY: Duration = Duration::from_millis(500);
    while !path.as_ref().exists() {
        thread::sleep(WAIT_DELAY);
    }
}
