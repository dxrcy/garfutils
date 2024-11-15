mod args;
mod random;

use std::fs::{self, DirEntry, File};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use chrono::{Datelike, NaiveDate};
use clap::Parser;
use rand::Rng;

macro_rules! command {
    ( async $name:expr, $( $arg:expr ),* $(,)? ) => {{
        let mut command = ::std::process::Command::new($name);
        $( command.arg($arg); )*
        command
            .spawn() // Spawn child process
            .with_context(|| format!(
                "Failed to spawn `{}` command with arguments: {:#?}",
                $name, command.get_args(),
            ))
    }};

    ( become $name:expr, $( $arg:expr ),* $(,)? ) => {{
        let mut command = ::std::process::Command::new($name);
        $( command.arg($arg); )*
        command
            // Inherit standard io streams (makes vim work)
            .stdin (::std::process::Stdio::inherit())
            .stdout(::std::process::Stdio::inherit())
            .stderr(::std::process::Stdio::inherit())
            .output() // Block execution
            .with_context(|| format!(
                "Failed to run `{}` command with arguments: {:#?}",
                $name, command.get_args(),
            ))
    }};

    ( $name:expr, $( $arg:expr ),* $(,)? ) => {{
        let mut command = ::std::process::Command::new($name);
        $( command.arg($arg); )*
        command
            .output() // Block execution
            .with_context(|| format!(
                "Failed to run `{}` command with arguments: {:#?}",
                $name, command.get_args(),
            ))
    }};
}

const ORIGINAL_COMICS_NAME: &str = "comics";
const GENERATED_POSTS_NAME: &str = "generated";
const COMPLETED_POSTS_NAME: &str = "completed";
const OLD_POSTS_NAME: &str = "old";
const LOCATION_NAME: &str = "garfutils";
// Not using `/tmp` to ensure same mount point as destination
const TEMP_NAME: &str = "tmp";
const CACHE_FILE_NAME: &str = "garfutils.recent";

const IMAGE_ESPERANTO_NAME: &str = "esperanto.png";
const IMAGE_ENGLISH_NAME: &str = "english.png";
const IMAGE_SVG_NAME: &str = "esperanto.svg";
const TRANSCRIPT_NAME: &str = "transcript";
const TITLE_NAME: &str = "title";
const DATE_NAME: &str = "date";
const PROPS_NAME: &str = "props";
const SPECIAL_NAME: &str = "special";

const IMAGE_CLASS_TRANSCRIBE: &str = "garfutils-transcribe";
const IMAGE_CLASS_SHOW: &str = "garfutils-show";

const ORIGINAL_COMIC_FORMAT: &str = "png";

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

const ICON_DATA: &[u8] = include_bytes!("../icon.png");

fn get_dir_config(location: Option<PathBuf>, cache_file: Option<PathBuf>) -> Result<DirConfig> {
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
        old_posts_dir: location.join(OLD_POSTS_NAME),
        temp_dir: location.join(TEMP_NAME),
        recently_shown_file: cache_file,
    };

    if !location.exists() || !location.is_dir() {
        bail!(
            "Location is not a directory: {:?}.\n\
            Please create the directory with sub-directories `comics`, `generated`, and `completed`, \
            each of which may be symlinks.",
            location
        );
    }

    for (path, name) in [
        (&dir_config.original_comics_dir, ORIGINAL_COMICS_NAME),
        (&dir_config.generated_posts_dir, GENERATED_POSTS_NAME),
        (&dir_config.completed_posts_dir, COMPLETED_POSTS_NAME),
        (&dir_config.old_posts_dir, OLD_POSTS_NAME),
        // Don't check temp directory
    ] {
        if !path.exists() || !path.is_dir() {
            bail!(
                "Location is missing sub-directory: `{0}`\n\
                in {1:?}\n\
                Please create the directory with sub-directories `{0}` which may be symlink.",
                name,
                location,
            );
        }
    }

    Ok(dir_config)
}

fn main() -> Result<()> {
    random::init_rng();

    let args = args::Args::parse();

    let dir_config = get_dir_config(args.location, args.cache_file)?;

    match args.command {
        args::Command::Show { date } => {
            show_comic(&dir_config, date)?;
        }

        args::Command::Make { date, recent } => {
            let date = get_date(&dir_config, date, recent).with_context(|| "Failed to get date")?;
            let name = get_unique_name(date);
            make_post(&dir_config, date, &name, false).with_context(|| "Failed to make post")?;
        }

        args::Command::Revise { id } => {
            let id = get_revise_id(&dir_config, id)?;
            revise_post(&dir_config, &id).with_context(|| "Failed to revise post")?;
            print_confirmation("Transcribe now? ");
            transcribe_post(&dir_config, &id).with_context(|| "Failed to transcribe post")?;
        }

        args::Command::Transcribe { id } => {
            let id = get_transcribe_id(&dir_config, id)?;
            transcribe_post(&dir_config, &id).with_context(|| "Failed to transcribe post")?;
        }
    }

    Ok(())
}

fn transcribe_post(dir_config: &DirConfig, id: &str) -> Result<()> {
    if !dir_config.temp_dir.exists() {
        fs::create_dir_all(&dir_config.temp_dir)
            .with_context(|| "Failed to create temp directory for transcript file")?;
    }

    // "{temp_dir}/transcript.{id}"
    let mut temp_file_path = dir_config.temp_dir.join("transcript");
    temp_file_path.set_extension(id);

    let transcript_file_path = dir_config
        .completed_posts_dir
        .join(id)
        .join(TRANSCRIPT_NAME);
    let esperanto_file_path = dir_config
        .completed_posts_dir
        .join(id)
        .join(IMAGE_ESPERANTO_NAME);
    let english_file_path = dir_config
        .completed_posts_dir
        .join(id)
        .join(IMAGE_ENGLISH_NAME);

    let id_number = id
        .parse::<u32>()
        .with_context(|| "Post id is not an integer")?;

    command!["pkill", "--full", IMAGE_CLASS_TRANSCRIBE]?;
    command![
        async "nsxiv",
        esperanto_file_path,
        english_file_path,
        "--class",
        IMAGE_CLASS_TRANSCRIBE,
    ]?;

    // ******** !!! BSPWM-SPECIFIC FUNCTIONALITY !!! ********
    thread::sleep(Duration::from_millis(100));

    let bspc_node = command!["bspc", "query", "-N", "-n"]?.stdout;
    let bspc_node = std::str::from_utf8(&bspc_node)
        .expect("commmand result should be utf-8")
        .trim();

    command!["tabc", "detach", bspc_node]?;
    thread::sleep(Duration::from_millis(100));

    command!["bspc", "node", "-s", "west"]?;
    command!["bspc", "node", "-z", "right", "-200", "0"]?;
    command!["bspc", "node", "-f", "east"]?;
    // ******************************************************

    let transcript_template = if transcript_file_path.exists() {
        println!("(transcript file already exists)");
        fs::read_to_string(&transcript_file_path)
            .with_context(|| "Failed to read existing transcript file")?
    } else {
        let is_sunday = (id_number + 1) % 7 == 0;
        if is_sunday {
            "---\n---\n---\n---\n---\n---"
        } else {
            "---\n---"
        }
        .to_string()
    };

    fs::write(&temp_file_path, &transcript_template)
        .with_context(|| "Failed to write template transcript file")?;

    command![become "nvim", &temp_file_path]?;

    command!["pkill", "--full", IMAGE_CLASS_TRANSCRIBE]?;

    if file_matches_string(&temp_file_path, &transcript_template)
        .with_context(|| "Failed to compare transcript file against previous version")?
    {
        println!("No changes made.");
        return Ok(());
    }

    print_confirmation("Save transcript file? ");

    fs::rename(temp_file_path, &transcript_file_path)
        .with_context(|| "Failed to move temporary file to save transcript")?;

    println!("Saved transcript file.");

    Ok(())
}

fn file_matches_string(file_path: impl AsRef<Path>, target: &str) -> io::Result<bool> {
    // TODO(opt): This doesn't have to alloc a new String
    let file_contents = fs::read_to_string(file_path)?;
    Ok(file_contents == target)
}

fn revise_post(dir_config: &DirConfig, id: &str) -> Result<()> {
    let date_file_path = dir_config.completed_posts_dir.join(id).join("date");
    let date_file = fs::read_to_string(date_file_path)?;
    let date = NaiveDate::parse_from_str(date_file.trim(), "%Y-%m-%d")
        .with_context(|| "Invalid date file for post")?;

    make_post(dir_config, date, id, true).with_context(|| "Failed to make post")?;

    let post_path = dir_config.completed_posts_dir.join(id);
    let generated_path = dir_config.generated_posts_dir.join(id);

    let copy_post_file = |file_name: &str, required: bool| -> Result<()> {
        let old_path = post_path.join(file_name);
        let new_path = generated_path.join(file_name);
        if !old_path.exists() {
            if !required {
                return Ok(());
            }
            bail!("Post is missing `{}` file", file_name);
        }
        fs::copy(old_path, new_path)
            .with_context(|| format!("Failed to copy `{}` file", file_name))?;
        Ok(())
    };

    copy_post_file(TITLE_NAME, true)?;
    for file_name in [TRANSCRIPT_NAME, PROPS_NAME, SPECIAL_NAME] {
        copy_post_file(file_name, false)?;
    }

    print_confirmation("Move old post to old directory? ");

    let old_post_path = dir_config.old_posts_dir.join(id);
    if old_post_path.exists() {
        bail!("TODO: post already revised");
    }
    fs::rename(&post_path, &old_post_path)
        .with_context(|| "Failed to move post to `old` directory")?;
    println!("Moved {} to old directory", id);

    println!("(waiting until done...)");
    wait_for_file(&post_path)?;

    Ok(())
}

fn print_confirmation(prompt: &str) {
    print!("{}", prompt);
    io::stdout().flush().expect("failed to flush stdout");
    stdin_read_and_discard();
}

fn wait_for_file(path: impl AsRef<Path>) -> Result<()> {
    const WAIT_DELAY: Duration = Duration::from_millis(500);
    while !path.as_ref().exists() {
        thread::sleep(WAIT_DELAY);
    }
    Ok(())
}

fn stdin_read_and_discard() {
    let mut reader = BufReader::new(io::stdin());
    loop {
        let mut buf = [0];
        reader.read_exact(&mut buf).expect("failed to read stdin");
        if buf[0] == b'\n' {
            return;
        }
    }
}

fn get_revise_id(dir_config: &DirConfig, id: Option<String>) -> Result<String> {
    if let Some(id) = id {
        if !post_exists(dir_config, &id) {
            bail!("No post exists with that id");
        }
        return Ok(id);
    }
    if let Some(id) =
        find_unrevised_post(dir_config).with_context(|| "Trying to find post to revise")?
    {
        return Ok(id);
    }
    bail!("No posts to revise");
}

fn get_transcribe_id(dir_config: &DirConfig, id: Option<String>) -> Result<String> {
    if let Some(id) = id {
        if !post_exists(dir_config, &id) {
            bail!("No post exists with that id");
        }
        return Ok(id);
    }
    if let Some(id) =
        find_untranscribed_post(dir_config).with_context(|| "Trying to find post to transcribe")?
    {
        return Ok(id);
    }
    bail!("No posts to transcribe");
}

fn post_exists(dir_config: &DirConfig, id: &str) -> bool {
    dir_config.completed_posts_dir.join(id).is_dir()
}

fn find_unrevised_post(dir_config: &DirConfig) -> Result<Option<String>> {
    if let Some(id) = find_post(&dir_config.completed_posts_dir, |path| {
        let svg_file_path = path.join(IMAGE_SVG_NAME);
        if svg_file_path.exists() {
            return Ok(false);
        }
        let props_file_path = path.join(PROPS_NAME);
        if !props_file_path.exists() {
            return Ok(false);
        }
        let props_file = fs::OpenOptions::new()
            .read(true)
            .open(&props_file_path)
            .with_context(|| "Failed to read props file")?;
        file_contains_line(props_file, "good")
    })? {
        return Ok(Some(id));
    }

    if let Some(id) = find_post(&dir_config.completed_posts_dir, |path| {
        let svg_file_path = path.join(IMAGE_SVG_NAME);
        Ok(!svg_file_path.exists())
    })? {
        return Ok(Some(id));
    }

    Ok(None)
}

fn find_untranscribed_post(dir_config: &DirConfig) -> Result<Option<String>> {
    if let Some(id) = find_post(&dir_config.completed_posts_dir, |path| {
        let transcript_file_path = path.join(TRANSCRIPT_NAME);
        let svg_file_path = path.join(IMAGE_SVG_NAME);
        Ok(!transcript_file_path.exists() && svg_file_path.exists())
    })? {
        return Ok(Some(id));
    }

    Ok(None)
}

fn find_post<F>(directory: impl AsRef<Path>, predicate: F) -> Result<Option<String>>
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
            .map(|name| name.to_string_lossy().to_string());
        let Some(file_name) = file_name.filter(|name| name.len() == 4) else {
            bail!("Post directory has invalid name");
        };

        return Ok(Some(file_name));
    }
    Ok(None)
}

fn file_contains_line(file: File, needle: &str) -> Result<bool> {
    let reader = io::BufReader::new(file);
    for line in reader.lines() {
        if line?.trim() == needle {
            return Ok(true);
        }
    }
    Ok(false)
}

fn get_unique_name(date: NaiveDate) -> String {
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

fn make_post(
    dir_config: &DirConfig,
    date: NaiveDate,
    name: &str,
    skip_post_check: bool,
) -> Result<()> {
    let mut original_comic_path = dir_config.original_comics_dir.join(date.to_string());
    original_comic_path.set_extension(ORIGINAL_COMIC_FORMAT);
    let generated_dir = dir_config.generated_posts_dir.join(name);
    // TODO(refactor): Define file names as constants
    let title_file_path = generated_dir.join(TITLE_NAME);
    let date_file_path = generated_dir.join(DATE_NAME);
    let generated_comic_path = generated_dir.join(IMAGE_ENGLISH_NAME);
    let duplicate_comic_path = generated_dir.join(IMAGE_ESPERANTO_NAME);

    let icon = image::load_from_memory(ICON_DATA).expect("load static icon as image");

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
    if exists_post_with_date(&dir_config.completed_posts_dir, date)? && !skip_post_check {
        bail!("There already exists a completed post with that date");
    }

    // Parent should already be created
    fs::create_dir(&generated_dir).with_context(|| "Failed to create generated post directory")?;

    fs::write(date_file_path, date.to_string()).with_context(|| "Failed to write date file")?;

    fs::File::create(title_file_path).with_context(|| "Failed to create title file")?;

    let original_comic =
        image::open(original_comic_path).with_context(|| "Failed to open comic")?;
    let generated_comic = comic_format::convert_image(original_comic, &icon, watermark, 0.0);

    generated_comic
        .save(&generated_comic_path)
        .with_context(|| "Failed to save generated image")?;

    fs::copy(&generated_comic_path, &duplicate_comic_path)
        .with_context(|| "Failed to copy generated image")?;

    println!("Created {}", name);

    Ok(())
}

fn get_random_watermark() -> &'static str {
    WATERMARKS[random::with_rng(|rng| rng.gen_range(0..WATERMARKS.len()))]
}

/// Skips entries with missing or malformed date file
fn exists_post_with_date(dir: impl AsRef<Path>, date: NaiveDate) -> Result<bool> {
    let entries = fs::read_dir(dir).with_context(|| "Failed to read directory")?;

    for entry in entries {
        let entry = entry.with_context(|| "Failed to read directory entry")?;

        let date_file_path = entry.path().join(DATE_NAME);
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

fn get_date(dir_config: &DirConfig, date: Option<NaiveDate>, recent: bool) -> Result<NaiveDate> {
    if !recent {
        return Ok(date.expect("date should be `Some` without `--recent` (cli parsing is broken)"));
    }
    assert!(
        date.is_none(),
        "date should be `None` with `--recent` (cli parsing is broken)"
    );
    get_recent_date(dir_config).with_context(|| "Failed to get recent date")
}

struct DirConfig {
    pub original_comics_dir: PathBuf,
    pub generated_posts_dir: PathBuf,
    pub completed_posts_dir: PathBuf,
    pub old_posts_dir: PathBuf,
    pub temp_dir: PathBuf,
    pub recently_shown_file: PathBuf,
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
                Err(error) => bail!("Cache file contains invalid date: {:?}", error),
            }
        }
    }
}

fn show_comic(dir_config: &DirConfig, date: Option<NaiveDate>) -> Result<()> {
    let (date, path) = match date {
        Some(date) => {
            let mut path = dir_config.original_comics_dir.join(date.to_string());
            path.set_extension(ORIGINAL_COMIC_FORMAT);
            (date, path)
        }
        None => {
            // TODO(fix): check if length == 0
            let path = get_random_directory_entry(&dir_config.original_comics_dir)
                .with_context(|| "Failed to read comics directory")?
                .path();
            let date = get_date_from_path(&path).with_context(|| {
                "Found comic file with invalid name. Should contain date in YYYY-MM-DD format."
            })?;
            (date, path)
        }
    };

    println!("{}", date);

    append_recent_date(dir_config, date).with_context(|| "Failed to append to cache file")?;

    command!["pkill", "--full", IMAGE_CLASS_SHOW]?;
    command![
        async "nsxiv",
        "--fullscreen",
        "--scale-mode",
        "f", // fit
        "--class",
        IMAGE_CLASS_SHOW,
        path,
    ]?;

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

fn get_random_directory_entry(directory: impl AsRef<Path>) -> io::Result<DirEntry> {
    let count = count_directory_entries(&directory)?;
    let index = random::with_rng(|rng| rng.gen_range(0..count));
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
