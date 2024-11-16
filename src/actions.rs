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
                .with_context(|| "Reading source directory")?
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
        .with_context(|| "Appending date to cache file")?;

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

    let icon = image::open(location.icon_file()).with_context(|| "Opening icon image")?;

    let watermark = get_random_watermark(location).with_context(|| "Parsing watermark")?;

    if !original_comic_path.exists() {
        bail!("Not the date of an existing comic");
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
    fs::create_dir(&output_dir).with_context(|| "Creating generated post directory")?;

    fs::write(date_file_path, date.to_string()).with_context(|| "Writing to date file")?;

    fs::File::create(title_file_path).with_context(|| "Creating title file")?;

    let original_comic = image::open(original_comic_path).with_context(|| "Opening comic image")?;
    let generated_comic = comic_format::convert_image(original_comic, &icon, &watermark, 0.0);

    generated_comic
        .save(&initial_path)
        .with_context(|| "Saving generated image")?;

    fs::copy(&initial_path, &duplicate_file_path).with_context(|| "Duplicating generated image")?;

    println!("Created {}", name);

    Ok(())
}

pub fn transcribe(location: &Location, id: &str) -> Result<()> {
    let temp_dir = location.temp_dir();
    if !temp_dir.exists() {
        fs::create_dir_all(&temp_dir)
            .with_context(|| "Creating temp directory for transcript file")?;
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
            .with_context(|| "Reading existing transcript file")?;
        Cow::from(contents)
    } else {
        Cow::from(if is_id_sunday(id)? {
            "---\n---\n---\n---\n---\n---"
        } else {
            "---\n---"
        })
    };

    fs::write(&temp_file_path, &*transcript_template)
        .with_context(|| "Writing template transcript file")?;

    commands::open_editor(&temp_file_path)?;

    commands::kill_process_class(viewer_class::TRANSCRIBE)?;

    if file::file_matches_string(&temp_file_path, &transcript_template)
        .with_context(|| "Comparing transcript file against previous version")?
    {
        println!("No changes made.");
        return Ok(());
    }

    confirm("Save transcript file?");

    fs::rename(temp_file_path, &transcript_file_path)
        .with_context(|| "Renaming temporary file as transcript file")?;

    println!("Saved transcript file.");

    Ok(())
}

pub fn revise(location: &Location, id: &str) -> Result<()> {
    let completed_dir = location.posts_dir();

    let date_file_path = completed_dir.join(id).join("date");
    let date_file = fs::read_to_string(date_file_path)?;
    let date = NaiveDate::parse_from_str(date_file.trim(), "%Y-%m-%d")
        .with_context(|| "Invalid date file for post")?;

    // TODO(refactor): Move to `main.rs`
    make(location, date, id, true).with_context(|| "Generating post")?;

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
            if is_required {
                bail!("Post is missing required `{}` file", file_name);
            }
        } else {
            fs::copy(old_path, new_path)
                .with_context(|| format!("Copying `{}` file", file_name))?;
        }
    }

    confirm("Move old post to old directory?");

    let old_post_path = location.old_dir().join(id);
    if old_post_path.exists() {
        // TODO(feat!): Handle post already revised
        bail!("unimplemented: post already revised");
    }
    fs::rename(&post_path, &old_post_path).with_context(|| "Moving post to `old` directory")?;
    println!("Moved {} to old directory", id);

    println!("(waiting until done...)");
    file::wait_for_file(&post_path)?;

    Ok(())
}

/// Skips entries with missing or malformed date file
fn exists_post_with_date(dir: impl AsRef<Path>, date: NaiveDate) -> Result<bool> {
    let entries = file::read_dir(&dir)?;

    for entry in entries {
        let entry = entry?;

        let date_file_path = entry.path().join(post_file::DATE);
        if !date_file_path.exists() {
            continue;
        }

        let date_file = fs::read_to_string(date_file_path).with_context(|| "Reading date file")?;
        let existing_date = NaiveDate::parse_from_str(date_file.trim(), "%Y-%m-%d")
            .with_context(|| "Parsing date in file")?;
        if existing_date == date {
            return Ok(true);
        }
    }

    Ok(false)
}

fn get_random_watermark(location: &Location) -> Result<String> {
    let contents = fs::read_to_string(location.watermarks_file())
        .with_context(|| "Reading watermarks file")?;
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
