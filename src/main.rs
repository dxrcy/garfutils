mod args;

use std::fs::{self, DirEntry};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::{Datelike, NaiveDate};
use clap::Parser;
use rand::Rng;

fn main() {
    let args = args::Args::parse();

    // TODO(feat): Use user's home directory
    // TODO(feat): Perhaps make customizable
    let dir_config = DirConfig {
        original_comics_dir: PathBuf::from("/home/darcy/pics/garfield"),
        recently_shown_file: PathBuf::from("/home/darcy/.cache/garfutils.recent"),
        generated_posts_dir: PathBuf::from("/home/darcy/pics/eo/unedited2"),
        completed_posts_dir: PathBuf::from("/home/darcy/code/garfeo/assets/posts"),
    };

    match args.command {
        args::Command::Show { date } => {
            show_comic(&dir_config, date);
        }

        args::Command::Make { date, recent, name } => {
            let date = get_date(&dir_config, date, recent);
            let name = name.unwrap_or_else(|| get_unique_name(date));
            make_post(&dir_config, date, &name);
            println!("Created {}", name);
        }

        args::Command::Revise { .. } => todo!(),

        args::Command::Transcribe { .. } => todo!(),
    }
}

fn get_unique_name(date: NaiveDate) -> String {
    use std::fmt::Write;

    const CODE_LENGTH: usize = 4;
    const STRING_LENGTH: usize = CODE_LENGTH + ":YYYY-mm-dd".len();

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

fn make_post(dir_config: &DirConfig, date: NaiveDate, name: &str) {
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
        // TODO(feat/error): Handle better
        panic!("not the date of a real comic");
    }

    if !dir_config.generated_posts_dir.exists() {
        // TODO(feat/error): Handle better
        fs::create_dir_all(&dir_config.generated_posts_dir).expect("failed to create directory");
    }

    if exists_post_with_date(&dir_config.generated_posts_dir, date) {
        panic!("already exists incomplete post with that date");
    }
    if exists_post_with_date(&dir_config.completed_posts_dir, date) {
        panic!("already exists completed post with that date");
    }

    // TODO(feat/error): Handle better
    // Parent should already be created
    fs::create_dir(&generated_dir).expect("failed to create directory");

    // TODO(feat/error): Handle better
    fs::write(date_file_path, date.to_string()).expect("failed to write date file");

    // TODO(feat/error): Handle better
    fs::File::create(title_file_path).expect("failed to create title file");

    // TODO(feat/error): Handle better
    let icon = image::open(icon_path).expect("failed to open icon image");
    // TODO(feat/error): Handle better
    let original_comic = image::open(original_comic_path).expect("failed to open comic");
    let generated_comic = comic_format::convert_image(original_comic, &icon, watermark, 0.0);

    // TODO(feat/error): Handle better
    generated_comic
        .save(&generated_comic_path)
        .expect("failed to save comic");

    // TODO(feat/error): Handle better
    fs::copy(&generated_comic_path, &duplicate_comic_path).expect("failed to copy comic");
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
fn exists_post_with_date(dir: impl AsRef<Path>, date: NaiveDate) -> bool {
    // TODO(feat/error): Handle better
    let entries = fs::read_dir(dir).expect("failed to read directory");

    for entry in entries {
        // TODO(feat/error): Handle better
        let entry = entry.expect("failed to read entry");

        let date_file_path = entry.path().join("date");
        if !date_file_path.exists() {
            continue;
        }

        // TODO(feat/error): Handle better
        let date_file = fs::read_to_string(date_file_path).expect("failed to read date file");
        let Ok(existing_date) = NaiveDate::parse_from_str(date_file.trim(), "%Y-%m-%d") else {
            continue;
        };
        if existing_date == date {
            return true;
        }
    }

    return false;
}

fn get_date(dir_config: &DirConfig, date: Option<NaiveDate>, recent: bool) -> NaiveDate {
    if !recent {
        return date.expect("date should be `Some` without `--recent` (cli parsing is broken)");
    }

    assert!(
        date.is_none(),
        "date should be `None` with `--recent` (cli parsing is broken)"
    );
    // TODO(feat/error): Handle better
    let Some(date) = get_recent_date(&dir_config).unwrap() else {
        // TODO(feat/error): Handle better
        panic!("no recent comic.");
    };
    return date;
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

fn get_recent_date(dir_config: &DirConfig) -> io::Result<Option<NaiveDate>> {
    if !dir_config.recently_shown_file.exists() {
        return Ok(None);
    }
    let file = fs::OpenOptions::new()
        .read(true)
        .open(&dir_config.recently_shown_file)?;
    let mut reader = BufReader::new(file);
    read_last_line_date(&mut reader)
}

fn read_last_line_date<R>(reader: &mut BufReader<R>) -> io::Result<Option<NaiveDate>>
where
    R: io::Read,
{
    let mut date: Option<NaiveDate> = None;
    loop {
        let mut new_line = String::new();
        let bytes_read = reader.read_line(&mut new_line)?;
        if bytes_read == 0 {
            return Ok(date);
        }
        if !new_line.trim().is_empty() {
            date = NaiveDate::parse_from_str(new_line.trim(), "%Y-%m-%d").ok();
        }
    }
}

fn show_comic(dir_config: &DirConfig, date: Option<NaiveDate>) {
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
            // TODO(feat/error): Handle better
            let path = get_random_directory_entry(&mut rng, &dir_config.original_comics_dir)
                .expect("failed to read comics directory")
                .path();
            // TODO(feat/error): Handle better
            let date = get_date_from_path(&path).expect("invalid path");
            (date, path)
        }
    };

    println!("{:?}", path);

    // TODO(feat/error): Handle better
    append_recent_date(dir_config, date).expect("failed to append to file");

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
    .unwrap(); // TODO(feat/error): Handle better
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
