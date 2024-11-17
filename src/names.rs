use crate::constants::*;
use crate::file;
use crate::location::Location;
use crate::random;

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use anyhow::{bail, Context as _, Result};
use chrono::{Datelike as _, NaiveDate};
use rand::Rng as _;

pub fn generate_name(date: NaiveDate) -> String {
    const CODE_LENGTH: usize = 4;
    const STRING_LENGTH: usize = CODE_LENGTH + ":YYYY-MM-DD".len();

    let mut name = String::with_capacity(STRING_LENGTH);

    let char_set = if date.weekday() == chrono::Weekday::Sun {
        'A'..='Z'
    } else {
        'a'..='z'
    };

    for _ in 0..CODE_LENGTH {
        let letter: char = random::with_rng(|rng| rng.gen_range(char_set.clone()));
        name.push(letter);
    }

    // Avoid unnecessary temporary string allocation
    write!(name, ":{}", date.format("%Y-%m-%d")).expect("write to string should not fail");
    name
}

pub fn get_date(location: &Location, date: Option<NaiveDate>, recent: bool) -> Result<NaiveDate> {
    if !recent {
        return Ok(date.expect("date should be `Some` without `--recent` (cli parsing is broken)"));
    }
    assert!(
        date.is_none(),
        "date should be `None` with `--recent` (cli parsing is broken)"
    );
    let recent_date = get_recent_date(location).with_context(|| "Parsing recent date")?;
    println!("Date: {}", recent_date);
    Ok(recent_date)
}

pub fn get_transcribe_id(location: &Location, id: Option<String>) -> Result<String> {
    if let Some(id) = id {
        if !location.posts_dir().join(&id).is_dir() {
            bail!("No post exists with that id");
        }
        return Ok(id);
    }
    if let Some(id) =
        find_untranscribed_post(location).with_context(|| "Finding post to transcribe")?
    {
        println!("Post id: {}", id);
        return Ok(id);
    }
    bail!("No posts to transcribe");
}

pub fn get_revise_id(location: &Location, id: Option<String>) -> Result<String> {
    if let Some(id) = id {
        if !location.posts_dir().join(&id).is_dir() {
            bail!("No post exists with that id");
        }
        return Ok(id);
    }
    if let Some(id) = find_unrevised_post(location).with_context(|| "Finding post to revise")? {
        println!("Post id: {}", id);
        return Ok(id);
    }
    bail!("No posts to revise");
}

pub fn read_date(location: &Location, id: &str) -> Result<NaiveDate> {
    let date_file_path = location.posts_dir().join(id).join("date");
    let date_file = fs::read_to_string(date_file_path)?;
    let date = NaiveDate::parse_from_str(date_file.trim(), "%Y-%m-%d")
        .with_context(|| "Invalid date file for post")?;
    Ok(date)
}

fn get_recent_date(location: &Location) -> Result<NaiveDate> {
    let recent_file = location.recent_file();

    if !recent_file.exists() {
        bail!("Recent dates file does not yet exist");
    }
    let file = fs::OpenOptions::new().read(true).open(&recent_file)?;
    file::read_last_line_as_date(file).with_context(|| "Reading recent date file")
}

fn find_untranscribed_post(location: &Location) -> Result<Option<String>> {
    find_post(
        location,
        [|path: &Path| Ok(has_svg_file(path) && !has_transcript_file(path))],
    )
}

fn find_unrevised_post(location: &Location) -> Result<Option<String>> {
    find_post(
        location,
        [
            |path: &Path| Ok(!has_svg_file(path) && is_post_good(path)?),
            |path: &Path| Ok(!has_svg_file(path)),
        ],
    )
}

fn has_svg_file(path: impl AsRef<Path>) -> bool {
    path.as_ref().join(post_file::SVG).exists()
}
fn has_transcript_file(path: impl AsRef<Path>) -> bool {
    path.as_ref().join(post_file::TRANSCRIPT).exists()
}

/// Returns `Ok(true)` if post has a `props` file, which contains the line `good`
fn is_post_good(path: impl AsRef<Path>) -> Result<bool> {
    const TARGET_LINE: &str = "good";

    let props_file_path = path.as_ref().join(post_file::PROPS);
    if !props_file_path.exists() {
        return Ok(false);
    }

    let props_file = fs::OpenOptions::new()
        .read(true)
        .open(&props_file_path)
        .with_context(|| format!("Opening `{}` file", post_file::PROPS))?;

    let has_target_line = file::file_contains_line(props_file, TARGET_LINE)
        .with_context(|| format!("Reading `{}` file", post_file::PROPS))?;

    Ok(has_target_line)
}

/// Loop through 'criteria' functions, until one finds an appropriate post
fn find_post<I, F>(location: &Location, criteria: I) -> Result<Option<String>>
where
    I: IntoIterator<Item = F>,
    F: Fn(&Path) -> Result<bool>,
{
    let posts_dir = location.posts_dir();
    for criterion in criteria {
        if let Some(id) = file::find_child(&posts_dir, criterion)? {
            return Ok(Some(id));
        }
    }
    Ok(None)
}
