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

pub fn get_random_directory_entry(dir: impl AsRef<Path>) -> Result<Option<DirEntry>> {
    let count = count_dir_entries(&dir).with_context(|| "Counting directory entries")?;
    if count == 0 {
        return Ok(None);
    }
    let index = random::with_rng(|rng| rng.gen_range(0..count));
    let entry = get_nth_dir_entry(&dir, index)?
        .expect("generated index should refer to a valid directory entry");
    Ok(Some(entry))
}

fn get_nth_dir_entry(dir: impl AsRef<Path>, index: usize) -> Result<Option<DirEntry>> {
    let mut entries = read_dir(dir)?;
    let Some(entry) = entries.nth(index) else {
        return Ok(None);
    };
    let entry = entry?;
    Ok(Some(entry))
}

fn count_dir_entries(dir: impl AsRef<Path>) -> Result<usize> {
    let mut count = 0;
    for entry in read_dir(&dir)? {
        entry?;
        count += 1;
    }
    Ok(count)
}

/// Wrapper for `fs::read_dir` which provides context for some errors
pub fn read_dir(dir: impl AsRef<Path>) -> Result<impl Iterator<Item = Result<DirEntry>>> {
    let entries =
        fs::read_dir(&dir).with_context(|| format!("Reading directory {:?}", dir.as_ref()))?;
    let entries = entries.map(|entry| entry.with_context(|| "Reading directory entry"));
    Ok(entries)
}

pub fn append_date(path: impl AsRef<Path>, date: NaiveDate) -> io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{}", date)?;
    Ok(())
}

pub fn get_date_from_path(path: impl AsRef<Path>) -> Option<NaiveDate> {
    let path = path.as_ref();
    let date_str = path.file_stem()?.to_string_lossy();
    let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d");
    date.ok()
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
    for entry in read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();

        if !predicate(&path)? {
            continue;
        }

        let file_name = path
            .file_name()
            // TODO(feat): Handle these sort of errors CONSISTENTLY
            .with_context(|| "Invalid file name")?
            .to_string_lossy()
            .to_string();

        return Ok(Some(file_name));
    }
    Ok(None)
}

pub fn file_matches_string(file_path: impl AsRef<Path>, target: &str) -> io::Result<bool> {
    // TODO(opt): This doesn't have to alloc a new String
    let file_contents = fs::read_to_string(file_path)?;
    Ok(file_contents == target)
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
