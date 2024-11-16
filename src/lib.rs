mod commands;
mod file;
mod location;
mod random;

use std::borrow::Cow;
use std::fs;
use std::io;
use std::io::Write as _;
use std::path::Path;

use anyhow::{bail, Context as _, Result};
use chrono::{Datelike as _, NaiveDate};
use rand::Rng as _;

pub use location::Location;
pub use random::init_rng;

const SOURCE_FORMAT: &str = "png";
mod post_file {
    pub const INITIAL: &str = "esperanto.png";
    pub const DUPLICATE: &str = "english.png";
    pub const SVG: &str = "esperanto.svg";
    pub const TITLE: &str = "title";
    pub const DATE: &str = "date";
    pub const TRANSCRIPT: &str = "transcript";
    pub const PROPS: &str = "props";
    pub const SPECIAL: &str = "special";
}
mod viewer_class {
    pub const TRANSCRIBE: &str = "garfutils-transcribe";
    pub const SHOW: &str = "garfutils-show";
}

pub fn show_comic(location: &Location, date: Option<NaiveDate>) -> Result<()> {
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

    file::append_recent_date(location.recent_file(), date)
        .with_context(|| "Failed to append to cache file")?;

    commands::kill_process_class(viewer_class::SHOW)?;
    commands::spawn_image_viewer(&[path], viewer_class::SHOW, true)?;

    Ok(())
}

pub fn make_post(
    location: &Location,
    date: NaiveDate,
    name: &str,
    skip_post_check: bool,
) -> Result<()> {
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

pub fn transcribe_post(location: &Location, id: &str) -> Result<()> {
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

    print_confirmation("Save transcript file?");

    fs::rename(temp_file_path, &transcript_file_path)
        .with_context(|| "Failed to move temporary file to save transcript")?;

    println!("Saved transcript file.");

    Ok(())
}

pub fn revise_post(location: &Location, id: &str) -> Result<()> {
    let completed_dir = location.posts_dir();

    let date_file_path = completed_dir.join(id).join("date");
    let date_file = fs::read_to_string(date_file_path)?;
    let date = NaiveDate::parse_from_str(date_file.trim(), "%Y-%m-%d")
        .with_context(|| "Invalid date file for post")?;

    make_post(location, date, id, true).with_context(|| "Failed to make post")?;

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

    print_confirmation("Move old post to old directory?");

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

pub fn get_unique_name(date: NaiveDate) -> String {
    use std::fmt::Write;

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

pub fn print_confirmation(prompt: &str) {
    print!("{} ", prompt);
    io::stdout().flush().expect("failed to flush stdout");
    file::stdin_read_and_discard();
}

fn get_recent_date(location: &Location) -> Result<NaiveDate> {
    let recent_file = location.recent_file();

    if !recent_file.exists() {
        bail!("Cache file does not yet exist");
    }
    let file = fs::OpenOptions::new().read(true).open(&recent_file)?;
    file::read_last_line_date(file)
}

fn get_random_watermark(location: &Location) -> Result<String> {
    let contents =
        fs::read_to_string(location.watermarks_file()).with_context(|| "Failed to read file")?;
    let watermarks: Vec<&str> = contents.lines().collect();
    let index = random::with_rng(|rng| rng.gen_range(0..watermarks.len()));
    Ok(watermarks[index].to_string())
}

fn find_unrevised_post(location: &Location) -> Result<Option<String>> {
    let completed_dir = location.posts_dir();

    if let Some(id) = file::find_child(&completed_dir, |path| {
        let svg_file_path = path.join(post_file::SVG);
        if svg_file_path.exists() {
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
        file::file_contains_line(props_file, "good")
    })? {
        return Ok(Some(id));
    }

    if let Some(id) = file::find_child(&completed_dir, |path| {
        let svg_file_path = path.join(post_file::SVG);
        Ok(!svg_file_path.exists())
    })? {
        return Ok(Some(id));
    }

    Ok(None)
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

fn is_id_sunday(id: &str) -> Result<bool> {
    let id_number = id
        .parse::<u32>()
        .with_context(|| "Post id is not an integer")?;
    let is_sunday = (id_number + 1) % 7 == 0;
    Ok(is_sunday)
}
