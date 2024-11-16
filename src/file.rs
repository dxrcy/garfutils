use crate::random;

use std::fs::{self, DirEntry, File};
use std::io::{self, BufRead as _, BufReader, Read as _, Write as _};
use std::path::Path;
use std::thread;
use std::time::Duration;

use anyhow::{bail, Context as _, Result};
use chrono::NaiveDate;
use rand::Rng as _;

pub fn get_date_from_path(path: impl AsRef<Path>) -> Option<NaiveDate> {
    let path = path.as_ref();
    let filename = path.file_name()?.to_string_lossy();
    let date_string = match filename.find('.') {
        Some(position) => filename.split_at(position).0,
        None => &filename,
    };
    let date = NaiveDate::parse_from_str(date_string, "%Y-%m-%d");
    date.ok()
}

pub fn append_recent_date(path: impl AsRef<Path>, date: NaiveDate) -> io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{}", date)?;
    Ok(())
}

pub fn get_random_directory_entry(directory: impl AsRef<Path>) -> io::Result<Option<DirEntry>> {
    let count = count_directory_entries(&directory)?;
    if count == 0 {
        return Ok(None);
    }
    let index = random::with_rng(|rng| rng.gen_range(0..count));
    let entry = get_nth_directory_entry(&directory, index)?
        .expect("generated index should be in range of directory entry count");
    Ok(Some(entry))
}

pub fn file_matches_string(file_path: impl AsRef<Path>, target: &str) -> io::Result<bool> {
    // TODO(opt): This doesn't have to alloc a new String
    let file_contents = fs::read_to_string(file_path)?;
    Ok(file_contents == target)
}

pub fn wait_for_file(path: impl AsRef<Path>) -> Result<()> {
    const WAIT_DELAY: Duration = Duration::from_millis(500);
    while !path.as_ref().exists() {
        thread::sleep(WAIT_DELAY);
    }
    Ok(())
}

pub fn find_child<F>(directory: impl AsRef<Path>, predicate: F) -> Result<Option<String>>
where
    F: Fn(&Path) -> Result<bool>,
{
    let entries =
        fs::read_dir(directory).with_context(|| "Failed to read completed posts directory")?;

    for entry in entries {
        let entry = entry.with_context(|| "Failed to read entry")?;
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

pub fn read_last_line_date(file: File) -> Result<NaiveDate> {
    let mut reader = BufReader::new(file);
    let mut date: Option<NaiveDate> = None;

    loop {
        let mut new_line = String::new();
        let bytes_read = reader.read_line(&mut new_line)?;
        if bytes_read == 0 {
            match date {
                Some(date) => return Ok(date),
                None => bail!("Cache file is empty"),
            }
        }
        if !new_line.trim().is_empty() {
            match NaiveDate::parse_from_str(new_line.trim(), "%Y-%m-%d") {
                Ok(new_date) => date = Some(new_date),
                Err(error) => bail!("Cache file contains invalid date: {}", error),
            }
        }
    }
}

pub fn file_contains_line(file: File, needle: &str) -> Result<bool> {
    let reader = io::BufReader::new(file);
    for line in reader.lines() {
        if line?.trim() == needle {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn stdin_read_and_discard() {
    let mut reader = BufReader::new(io::stdin());
    let mut buf = [0];
    loop {
        reader.read_exact(&mut buf).expect("failed to read stdin");
        if buf[0] == b'\n' {
            return;
        }
    }
}

fn get_nth_directory_entry(
    directory: impl AsRef<Path>,
    index: usize,
) -> io::Result<Option<DirEntry>> {
    let mut entries = fs::read_dir(directory)?;
    let Some(entry) = entries.nth(index) else {
        return Ok(None);
    };
    let entry = entry?;
    Ok(Some(entry))
}

fn count_directory_entries(directory: impl AsRef<Path>) -> io::Result<usize> {
    let entries = fs::read_dir(directory)?;
    let mut count = 0;
    for entry in entries {
        entry?;
        count += 1;
    }
    Ok(count)
}
