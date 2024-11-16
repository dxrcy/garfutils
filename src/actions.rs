use crate::commands;
use crate::confirm;
use crate::constants::*;
use crate::file;
use crate::location::Location;
use crate::random;

use std::borrow::Cow;
use std::fs;
use std::path::Path;

use anyhow::{bail, Context as _, Result};
use chrono::NaiveDate;
use rand::Rng as _;

pub fn show(location: &Location, date: Option<NaiveDate>) -> Result<()> {
    let source_dir = location.source_dir();

    let (date, path) = match date {
        Some(date) => {
            let mut path = source_dir.join(date.to_string());
            path.set_extension(SOURCE_FORMAT);
            (date, path)
        }
        None => {
            let path = file::get_random_directory_entry(&source_dir)
                .with_context(|| "Failed to read comics directory")?
                .with_context(|| "No comics found")?
                .path();
            let date = file::get_date_from_path(&path).with_context(|| {
                "Found comic file with invalid name. Should contain date in YYYY-MM-DD format."
            })?;
            (date, path)
        }
    };

    println!("{}", date);

    file::append_date(location.recent_file(), date)
        .with_context(|| "Failed to append to cache file")?;

    commands::kill_process_class(viewer_class::SHOW)?;
    commands::spawn_image_viewer(&[path], viewer_class::SHOW, true)?;

    Ok(())
}

pub fn make(location: &Location, date: NaiveDate, name: &str, skip_post_check: bool) -> Result<()> {
    let generated_dir = location.generated_dir();

    let mut original_comic_path = location.source_dir().join(date.to_string());
    original_comic_path.set_extension(SOURCE_FORMAT);
    let output_dir = generated_dir.join(name);
    let title_file_path = output_dir.join(post_file::TITLE);
    let date_file_path = output_dir.join(post_file::DATE);
    let initial_path = output_dir.join(post_file::INITIAL);
    let duplicate_file_path = output_dir.join(post_file::DUPLICATE);

    let icon = image::open(location.icon_file()).with_context(|| "Failed to open icon image")?;

    let watermark = get_random_watermark(location).with_context(|| "Failed to get watermark")?;

    if !original_comic_path.exists() {
        bail!("Not the date of a real comic");
    }

    if exists_post_with_date(&generated_dir, date)
        .with_context(|| "Checking if post already generated")?
    {
        bail!("There already exists an incomplete post with that date");
    }
    if exists_post_with_date(location.posts_dir(), date)
        .with_context(|| "Checking if post already exists")?
        && !skip_post_check
    {
        bail!("There already exists a completed post with that date");
    }

    // Parent should already be created
    fs::create_dir(&output_dir).with_context(|| "Failed to create generated post directory")?;

    fs::write(date_file_path, date.to_string()).with_context(|| "Failed to write date file")?;

    fs::File::create(title_file_path).with_context(|| "Failed to create title file")?;

    let original_comic =
        image::open(original_comic_path).with_context(|| "Failed to open comic")?;
    let generated_comic = comic_format::convert_image(original_comic, &icon, &watermark, 0.0);

    generated_comic
        .save(&initial_path)
        .with_context(|| "Failed to save generated image")?;

    fs::copy(&initial_path, &duplicate_file_path)
        .with_context(|| "Failed to copy generated image")?;

    println!("Created {}", name);

    Ok(())
}

pub fn transcribe(location: &Location, id: &str) -> Result<()> {
    let temp_dir = location.temp_dir();
    if !temp_dir.exists() {
        fs::create_dir_all(&temp_dir)
            .with_context(|| "Failed to create temp directory for transcript file")?;
    }

    // "{temp_dir}/transcript.{id}"
    let mut temp_file_path = temp_dir.join("transcript");
    temp_file_path.set_extension(id);

    let completed_dir = location.posts_dir().join(id);

    let transcript_file_path = completed_dir.join(post_file::TRANSCRIPT);
    let initial_file_path = completed_dir.join(post_file::INITIAL);
    let duplicate_file_path = completed_dir.join(post_file::DUPLICATE);

    commands::kill_process_class(viewer_class::TRANSCRIBE)?;

    commands::setup_image_viewer_window(
        &[initial_file_path, duplicate_file_path],
        viewer_class::TRANSCRIBE,
    )?;

    let transcript_template = if transcript_file_path.exists() {
        println!("(transcript file already exists)");
        let contents = fs::read_to_string(&transcript_file_path)
            .with_context(|| "Failed to read existing transcript file")?;
        Cow::from(contents)
    } else {
        Cow::from(if is_id_sunday(id)? {
            "---\n---\n---\n---\n---\n---"
        } else {
            "---\n---"
        })
    };

    fs::write(&temp_file_path, &*transcript_template)
        .with_context(|| "Failed to write template transcript file")?;

    commands::open_editor(&temp_file_path)?;

    commands::kill_process_class(viewer_class::TRANSCRIBE)?;

    if file::file_matches_string(&temp_file_path, &transcript_template)
        .with_context(|| "Failed to compare transcript file against previous version")?
    {
        println!("No changes made.");
        return Ok(());
    }

    confirm("Save transcript file?");

    fs::rename(temp_file_path, &transcript_file_path)
        .with_context(|| "Failed to move temporary file to save transcript")?;

    println!("Saved transcript file.");

    Ok(())
}

pub fn revise(location: &Location, id: &str) -> Result<()> {
    let completed_dir = location.posts_dir();

    let date_file_path = completed_dir.join(id).join("date");
    let date_file = fs::read_to_string(date_file_path)?;
    let date = NaiveDate::parse_from_str(date_file.trim(), "%Y-%m-%d")
        .with_context(|| "Invalid date file for post")?;

    make(location, date, id, true).with_context(|| "Failed to make post")?;

    let post_path = completed_dir.join(id);
    let generated_path = location.generated_dir().join(id);

    let copy_files = [
        (post_file::TITLE, true),
        (post_file::TRANSCRIPT, false),
        (post_file::PROPS, false),
        (post_file::SPECIAL, false),
        (post_file::SVG, false),
        // Date and PNG images already created
    ];
    for (file_name, is_required) in copy_files {
        let old_path = post_path.join(file_name);
        let new_path = generated_path.join(file_name);
        if !old_path.exists() {
            if !is_required {
                continue;
            }
            bail!("Post is missing `{}` file", file_name);
        }
        fs::copy(old_path, new_path)
            .with_context(|| format!("Failed to copy `{}` file", file_name))?;
    }

    confirm("Move old post to old directory?");

    let old_post_path = location.old_dir().join(id);
    if old_post_path.exists() {
        // TODO(feat): Handle post already revised
        bail!("unimplemented: post already revised");
    }
    fs::rename(&post_path, &old_post_path)
        .with_context(|| "Failed to move post to `old` directory")?;
    println!("Moved {} to old directory", id);

    println!("(waiting until done...)");
    file::wait_for_file(&post_path)?;

    Ok(())
}

/// Skips entries with missing or malformed date file
fn exists_post_with_date(dir: impl AsRef<Path>, date: NaiveDate) -> Result<bool> {
    let entries = fs::read_dir(dir).with_context(|| "Failed to read directory")?;

    for entry in entries {
        let entry = entry.with_context(|| "Failed to read directory entry")?;

        let date_file_path = entry.path().join(post_file::DATE);
        if !date_file_path.exists() {
            continue;
        }

        let date_file =
            fs::read_to_string(date_file_path).with_context(|| "Failed to read date file")?;
        let Ok(existing_date) = NaiveDate::parse_from_str(date_file.trim(), "%Y-%m-%d") else {
            continue;
        };
        if existing_date == date {
            return Ok(true);
        }
    }

    Ok(false)
}

fn get_random_watermark(location: &Location) -> Result<String> {
    let contents =
        fs::read_to_string(location.watermarks_file()).with_context(|| "Failed to read file")?;
    let watermarks: Vec<&str> = contents.lines().collect();
    let index = random::with_rng(|rng| rng.gen_range(0..watermarks.len()));
    Ok(watermarks[index].to_string())
}

fn is_id_sunday(id: &str) -> Result<bool> {
    let id_number = id
        .parse::<u32>()
        .with_context(|| "Post id is not an integer")?;
    let is_sunday = (id_number + 1) % 7 == 0;
    Ok(is_sunday)
}
