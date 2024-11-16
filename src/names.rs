use crate::constants::*;
use crate::file;
use crate::location::Location;
use crate::random;

use std::fs;

use anyhow::{bail, Context as _, Result};
use chrono::{Datelike as _, NaiveDate};
use rand::Rng as _;
use std::fmt::Write as _;

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
    let recent_date = get_recent_date(location).with_context(|| "Failed to get recent date")?;
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
        find_untranscribed_post(location).with_context(|| "Trying to find post to transcribe")?
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
    if let Some(id) =
        find_unrevised_post(location).with_context(|| "Trying to find post to revise")?
    {
        println!("Post id: {}", id);
        return Ok(id);
    }
    bail!("No posts to revise");
}

fn get_recent_date(location: &Location) -> Result<NaiveDate> {
    let recent_file = location.recent_file();

    if !recent_file.exists() {
        bail!("Cache file does not yet exist");
    }
    let file = fs::OpenOptions::new().read(true).open(&recent_file)?;
    file::read_last_line_as_date(file)
}

fn find_untranscribed_post(location: &Location) -> Result<Option<String>> {
    let completed_dir = location.posts_dir();

    if let Some(id) = file::find_child(&completed_dir, |path| {
        let transcript_file_path = path.join(post_file::TRANSCRIPT);
        let svg_file_path = path.join(post_file::SVG);
        Ok(!transcript_file_path.exists() && svg_file_path.exists())
    })? {
        return Ok(Some(id));
    }

    Ok(None)
}

fn find_unrevised_post(location: &Location) -> Result<Option<String>> {
    let completed_dir = location.posts_dir();

    if let Some(id) = file::find_child(&completed_dir, |path| {
        if path.join(post_file::SVG).exists() {
            return Ok(false);
        }
        let props_file_path = path.join(post_file::PROPS);
        if !props_file_path.exists() {
            return Ok(false);
        }
        let props_file = fs::OpenOptions::new()
            .read(true)
            .open(&props_file_path)
            .with_context(|| "Failed to read props file")?;
        if !file::file_contains_line(props_file, "good")? {
            return Ok(false);
        }
        Ok(false)
    })? {
        return Ok(Some(id));
    }

    if let Some(id) = file::find_child(&completed_dir, |path| {
        if path.join(post_file::SVG).exists() {
            return Ok(false);
        }
        Ok(true)
    })? {
        return Ok(Some(id));
    }

    if let Some(id) = file::find_child(&completed_dir, |path| {
        if !path.join(post_file::SVG).exists() {
            return Ok(false);
        }
        if path.join(post_file::TRANSCRIPT).exists() {
            return Ok(false);
        }
        Ok(true)
    })? {
        return Ok(Some(id));
    }

    Ok(None)
}
