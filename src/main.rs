mod args;

use std::fs::{self, DirEntry};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use chrono::{Datelike, NaiveDate};
use clap::Parser;
use rand::Rng;

fn get_dir_config(location: Option<PathBuf>, cache_file: Option<PathBuf>) -> Result<DirConfig> {
    const ORIGINAL_COMICS_NAME: &str = "comics";
    const GENERATED_POSTS_NAME: &str = "generated";
    const COMPLETED_POSTS_NAME: &str = "completed";
    const LOCATION_NAME: &str = "garfutils";
    const CACHE_FILE_NAME: &str = "garfutils.recent";

    let Some(location) =
        location.or_else(|| dirs_next::data_dir().map(|dir| dir.join(LOCATION_NAME)))
    else {
        bail!(
            "Failed to read standard data location.\n\
            For *nix systems, try setting `$XDG_DATA_HOME` or `$HOME` environment variables.\n\
            Alternatively, run this program with the `--location <LOCATION>` option."
        );
    };

    let Some(cache_file) =
        cache_file.or_else(|| dirs_next::cache_dir().map(|dir| dir.join(CACHE_FILE_NAME)))
    else {
        bail!(
            "Failed to read standard cache location.\n\
            For *nix systems, try setting `$XDG_CACHE_HOME` or `$HOME` environment variables.\n\
            Alternatively, run this program with the `--cache-file <CACHE_FILE>` option."
        );
    };

    let dir_config = DirConfig {
        original_comics_dir: location.join(ORIGINAL_COMICS_NAME),
        generated_posts_dir: location.join(GENERATED_POSTS_NAME),
        completed_posts_dir: location.join(COMPLETED_POSTS_NAME),
        recently_shown_file: cache_file,
    };

    if !location.exists() || !location.is_dir() {
        bail!(
            "Location is not a directory: `{:?}`.\n\
            Please create the directory with sub-directories `comics`, `generated`, and `completed`, \
            each of which may be symlinks.",
            location
        );
    }

    for (path, name) in [
        (&dir_config.original_comics_dir, ORIGINAL_COMICS_NAME),
        (&dir_config.generated_posts_dir, GENERATED_POSTS_NAME),
        (&dir_config.completed_posts_dir, COMPLETED_POSTS_NAME),
    ] {
        if !path.exists() || !path.is_dir() {
            bail!(
                "Location is missing sub-directory: `{0}`\n\
                Please create the directory with sub-directories `{0}` which may be symlink.",
                name,
            );
        }
    }

    Ok(dir_config)
}

fn main() -> Result<()> {
    let args = args::Args::parse();

    let dir_config = get_dir_config(args.location, args.cache_file)?;

    match args.command {
        args::Command::Show { date } => {
            show_comic(&dir_config, date)?;
        }

        args::Command::Make { date, recent, name } => {
            let date = get_date(&dir_config, date, recent)?;
            let name = name.unwrap_or_else(|| get_unique_name(date));
            make_post(&dir_config, date, &name)?;
            println!("Created {}", name);
        }

        args::Command::Revise { .. } => {}

        args::Command::Transcribe { .. } => todo!(),
    }

    Ok(())
}

fn get_unique_name(date: NaiveDate) -> String {
    use std::fmt::Write;

    const CODE_LENGTH: usize = 4;
    const STRING_LENGTH: usize = CODE_LENGTH + ":YYYY-MM-DD".len();

    // TODO(refactor): Share rng
    let mut rng = rand::thread_rng();

    let mut name = String::with_capacity(STRING_LENGTH);

    let char_set = if date.weekday() == chrono::Weekday::Sun {
        'A'..='Z'
    } else {
        'a'..='z'
    };

    for _ in 0..CODE_LENGTH {
        let letter: char = rng.gen_range(char_set.clone());
        name.push(letter);
    }

    // Avoid unnecessary temporary string allocation
    write!(name, ":{}", date.format("%Y-%m-%d")).expect("write to string should not fail");

    name
}

fn make_post(dir_config: &DirConfig, date: NaiveDate, name: &str) -> Result<()> {
    let original_comic_path = dir_config
        .original_comics_dir
        .join(date.to_string() + ".png");
    let generated_dir = dir_config.generated_posts_dir.join(name);
    // TODO(refactor): Define file names as constants
    let title_file_path = generated_dir.join("title");
    let date_file_path = generated_dir.join("date");
    let generated_comic_path = generated_dir.join("english.png");
    let duplicate_comic_path = generated_dir.join("esperanto.png");
    // TODO(feat): Move icon file: Either to this crate or some location defined by dir_config
    let icon_path = Path::new("../comic-format/icon.png");

    let watermark = get_random_watermark();

    if !original_comic_path.exists() {
        bail!("Not the date of a real comic");
    }

    if !dir_config.generated_posts_dir.exists() {
        fs::create_dir_all(&dir_config.generated_posts_dir)
            .with_context(|| "Failed to create generated posts directory")?;
    }

    if exists_post_with_date(&dir_config.generated_posts_dir, date)? {
        bail!("There already exists an incomplete post with that date");
    }
    if exists_post_with_date(&dir_config.completed_posts_dir, date)? {
        bail!("There already exists a completed post with that date");
    }

    // Parent should already be created
    fs::create_dir(&generated_dir).with_context(|| "Failed to create generated post directory")?;

    fs::write(date_file_path, date.to_string()).with_context(|| "Failed to write date file")?;

    fs::File::create(title_file_path).with_context(|| "Failed to create title file")?;

    let icon = image::open(icon_path).with_context(|| "Failed to open icon image")?;
    let original_comic =
        image::open(original_comic_path).with_context(|| "Failed to open comic")?;
    let generated_comic = comic_format::convert_image(original_comic, &icon, watermark, 0.0);

    generated_comic
        .save(&generated_comic_path)
        .with_context(|| "Failed to save generated image")?;

    fs::copy(&generated_comic_path, &duplicate_comic_path)
        .with_context(|| "Failed to copy generated image")?;

    Ok(())
}

fn get_random_watermark() -> &'static str {
    // TODO(refactor): Share rng
    let mut rng = rand::thread_rng();

    const WATERMARKS: &[&str] = &[
        "GarfEO",
        "@garfield.eo.v2",
        "@garfieldeo@mastodon.world",
        "Garfield-EO",
        "garfeo",
        "Garfeo",
        "Garfield Esperanto",
        "Garfildo Esperanta",
        "Esperanta Garfield",
        "garf-eo",
    ];

    return WATERMARKS[rng.gen_range(0..WATERMARKS.len())];
}

/// Skips entries with missing or malformed date file
fn exists_post_with_date(dir: impl AsRef<Path>, date: NaiveDate) -> Result<bool> {
    let entries = fs::read_dir(dir).with_context(|| "Failed to read directory")?;

    for entry in entries {
        let entry = entry.with_context(|| "Failed to read directory entry")?;

        let date_file_path = entry.path().join("date");
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

    return Ok(false);
}

fn get_date(dir_config: &DirConfig, date: Option<NaiveDate>, recent: bool) -> Result<NaiveDate> {
    if !recent {
        return Ok(date.expect("date should be `Some` without `--recent` (cli parsing is broken)"));
    }

    assert!(
        date.is_none(),
        "date should be `None` with `--recent` (cli parsing is broken)"
    );

    get_recent_date(&dir_config).with_context(|| "Failed to get recent date")
}

struct DirConfig {
    pub original_comics_dir: PathBuf,
    pub recently_shown_file: PathBuf,
    pub generated_posts_dir: PathBuf,
    pub completed_posts_dir: PathBuf,
}

macro_rules! command {
    (
        $name:expr, $( $arg:expr ),* $(,)?
    ) => {{
        Command::new($name)
            $( .arg($arg) )*
    }};
}

fn get_recent_date(dir_config: &DirConfig) -> Result<NaiveDate> {
    if !dir_config.recently_shown_file.exists() {
        bail!("Cache file does not yet exist");
    }
    let file = fs::OpenOptions::new()
        .read(true)
        .open(&dir_config.recently_shown_file)?;
    let mut reader = BufReader::new(file);
    read_last_line_date(&mut reader)
}

fn read_last_line_date<R>(reader: &mut BufReader<R>) -> Result<NaiveDate>
where
    R: io::Read,
{
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
                Err(error) => bail!("Cache file contains invalid date: `{:?}`", error),
            }
        }
    }
}

fn show_comic(dir_config: &DirConfig, date: Option<NaiveDate>) -> Result<()> {
    // TODO(refactor): Share rng
    let mut rng = rand::thread_rng();

    let (date, path) = match date {
        Some(date) => (
            date,
            dir_config
                .original_comics_dir
                .join(date.to_string() + ".png"),
        ),
        None => {
            let path = get_random_directory_entry(&mut rng, &dir_config.original_comics_dir)
                .with_context(|| "Failed to read comics directory")?
                .path();
            let date = get_date_from_path(&path).with_context(|| {
                "Found comic file with invalid name. Should contain date in YYYY-MM-DD format."
            })?;
            (date, path)
        }
    };

    println!("{:?}", path);

    append_recent_date(dir_config, date).with_context(|| "Failed to append to cache file")?;

    command!(
        "nsxiv",
        "--fullscreen",
        "--scale-mode",
        "f", // fit
        "--class",
        "garfutils-show",
        path,
    )
    .spawn()
    .with_context(|| "Failed to open image viewer")?;

    Ok(())
}

fn get_date_from_path(path: impl AsRef<Path>) -> Option<NaiveDate> {
    let path = path.as_ref();
    let filename = path.file_name()?.to_string_lossy();
    let date_string = match filename.find('.') {
        Some(position) => filename.split_at(position).0,
        None => &filename,
    };
    let date = NaiveDate::parse_from_str(date_string, "%Y-%m-%d");
    date.ok()
}

fn append_recent_date(dir_config: &DirConfig, date: NaiveDate) -> io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&dir_config.recently_shown_file)?;
    writeln!(file, "{}", date)?;
    Ok(())
}

fn get_random_directory_entry(
    rng: &mut impl Rng,
    directory: impl AsRef<Path>,
) -> io::Result<DirEntry> {
    let count = count_directory_entries(&directory)?;
    let index = rng.gen_range(0..count);
    let entry = get_nth_directory_entry(&directory, index)?
        .expect("generated index should be in range of directory entry count");
    Ok(entry)
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
