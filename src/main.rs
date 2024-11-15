mod args;
mod random;

use std::borrow::Cow;
use std::ffi::OsStr;
use std::fs::{self, DirEntry, File};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{self, Command, Stdio};
use std::thread;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use chrono::{Datelike, NaiveDate};
use clap::Parser;
use rand::Rng;

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

struct Location {
    base_dir: PathBuf,
}

impl Location {
    const DEFAULT_LOCATION_NAME: &str = "garfutils"; // $XDG_DATA_DIR/<name>/
    const SOURCE_DIR: &str = "source";
    const GENERATED_DIR: &str = "generated";
    const POSTS_DIR: &str = "posts";
    const OLD_DIR: &str = "old";
    const TEMP_DIR: &str = "tmp"; // Not using `/tmp` to ensure same mount point as destination
    const RECENT_FILE: &str = "recent";
    const WATERMARKS_FILE: &str = "watermarks";
    const ICON_FILE: &str = "icon.png";

    pub fn from(base_dir: Option<PathBuf>) -> Result<Self> {
        let base_dir = Self::get_base_dir(base_dir)?;
        let location = Self { base_dir };
        location.check_dirs_exist()?;
        Ok(location)
    }

    pub fn source_dir(&self) -> PathBuf {
        self.base_dir.join(Self::SOURCE_DIR)
    }
    pub fn generated_dir(&self) -> PathBuf {
        self.base_dir.join(Self::GENERATED_DIR)
    }
    pub fn posts_dir(&self) -> PathBuf {
        self.base_dir.join(Self::POSTS_DIR)
    }
    pub fn old_dir(&self) -> PathBuf {
        self.base_dir.join(Self::OLD_DIR)
    }
    pub fn temp_dir(&self) -> PathBuf {
        self.base_dir.join(Self::TEMP_DIR)
    }
    pub fn recent_file(&self) -> PathBuf {
        self.base_dir.join(Self::RECENT_FILE)
    }
    pub fn watermarks_file(&self) -> PathBuf {
        self.base_dir.join(Self::WATERMARKS_FILE)
    }
    pub fn icon_file(&self) -> PathBuf {
        self.base_dir.join(Self::ICON_FILE)
    }

    fn get_base_dir(base_dir: Option<PathBuf>) -> Result<PathBuf> {
        if let Some(path) = base_dir {
            return Ok(path);
        }
        if let Some(path) = dirs_next::data_dir() {
            return Ok(path.join(Self::DEFAULT_LOCATION_NAME));
        }
        bail!(
            "Failed to read standard data location.\n\
            For *nix systems, try setting `$XDG_DATA_HOME` or `$HOME` environment variables.\n\
            Alternatively, run this program with the `--location <LOCATION>` option."
        );
    }

    fn check_dirs_exist(&self) -> Result<()> {
        if !self.base_dir.is_dir() {
            bail!(
                "Location is not a directory: `{}`.\n{}",
                self.base_dir.to_string_lossy(),
                self.format_dir_structure(),
            );
        }

        // TODO(opt): Use function pointers?
        let expected_sub_dirs = [
            (self.source_dir(), Self::SOURCE_DIR, true),
            (self.generated_dir(), Self::GENERATED_DIR, true),
            (self.posts_dir(), Self::POSTS_DIR, true),
            (self.old_dir(), Self::OLD_DIR, true),
            (self.watermarks_file(), Self::WATERMARKS_FILE, false),
            (self.icon_file(), Self::ICON_FILE, false),
        ];
        for (path, name, is_dir) in expected_sub_dirs {
            let is_correct_kind = if is_dir {
                path.is_dir()
            } else {
                path.is_file()
            };
            if !is_correct_kind {
                bail!(
                    "Location is missing {}: `{}`\n{}",
                    if is_dir { "sub-directory" } else { "file" },
                    name,
                    self.format_dir_structure()
                );
            }
        }

        Ok(())
    }

    fn format_dir_structure(&self) -> String {
        format!(
            "\
                \n\
                Please ensure that these files and directories exist.\n\
                Each item may be a symlink.\n\
                \n\
                \x1b[4m{}/\x1b[0m\n\
                    \t├─ {}/\n\
                    \t├─ {}/\n\
                    \t├─ {}/\n\
                    \t├─ {}\n\
                    \t└─ {}\n\
                \n\
                Alternatively, run this program with the `--location <LOCATION>` option.\
            ",
            self.base_dir.to_string_lossy(),
            Self::SOURCE_DIR,
            Self::GENERATED_DIR,
            Self::POSTS_DIR,
            Self::WATERMARKS_FILE,
            Self::ICON_FILE,
        )
    }
}

fn main() -> Result<()> {
    random::init_rng();

    let args = args::Args::parse();

    let location = Location::from(args.location).with_context(|| "Failed to verify location")?;

    match args.command {
        args::Command::Show { date } => {
            show_comic(&location, date)?;
        }

        args::Command::Make { date, recent } => {
            let date = get_date(&location, date, recent).with_context(|| "Failed to get date")?;
            let name = get_unique_name(date);
            make_post(&location, date, &name, false).with_context(|| "Failed to make post")?;
        }

        args::Command::Revise { id } => {
            let id = get_revise_id(&location, id)?;
            revise_post(&location, &id).with_context(|| "Failed to revise post")?;
            print_confirmation("Transcribe now?");
            transcribe_post(&location, &id).with_context(|| "Failed to transcribe post")?;
        }

        args::Command::Transcribe { id } => {
            let id = get_transcribe_id(&location, id)?;
            transcribe_post(&location, &id).with_context(|| "Failed to transcribe post")?;
        }
    }

    Ok(())
}

fn spawn_image_viewer(paths: &[impl AsRef<OsStr>], class: &str, fullscreen: bool) -> Result<()> {
    let mut command = Command::new("nsxiv");
    command.arg("--class").arg(class);
    if fullscreen {
        command.args([
            "--fullscreen",
            "--scale-mode",
            "f", // fit
        ]);
    }
    command
        .args(paths)
        .spawn()
        .with_context(|| "Failed to spawn image viewer")?;
    Ok(())
}

fn kill_process_class(class: &str) -> Result<()> {
    Command::new("pkill")
        .arg("--full")
        .arg(class)
        .status()
        .with_context(|| "Failed to kill image viewer")?;
    Ok(())
}

fn open_editor(path: impl AsRef<OsStr>) -> Result<()> {
    Command::new("nvim")
        .arg(path)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| "Failed to open editor")?;
    Ok(())
}

fn bspc_command(args: &[impl AsRef<OsStr>]) -> Result<process::Output> {
    let output = Command::new("bspc")
        .args(args)
        .output()
        .with_context(|| "Failed to run bspc command")?;
    Ok(output)
}

/// BSPWM-specific functionality
fn setup_image_viewer_window(paths: &[impl AsRef<OsStr>]) -> Result<()> {
    // Window ID of main window (terminal)
    let bspc_node = bspc_command(&["query", "-N", "-n"])?.stdout;
    let bspc_node = std::str::from_utf8(&bspc_node)
        .expect("commmand result should be utf-8")
        .trim();

    // Temporary hide currently focused window
    // To avoid attaching image viewer to `tabbed` instance
    bspc_command(&["node", bspc_node, "-g", "hidden"])?;

    spawn_image_viewer(paths, IMAGE_CLASS_TRANSCRIBE, false)?;
    // Wait for image viewer to completely start
    thread::sleep(Duration::from_millis(50));

    // Unhide main window
    // Move image viewer to left, resize slightly, re-focus main window
    bspc_command(&["node", bspc_node, "-g", "hidden"])?;
    bspc_command(&["node", "-s", "west"])?;
    bspc_command(&["node", "-z", "right", "-200", "0"])?;
    bspc_command(&["node", "-f", "east"])?;

    Ok(())
}

fn transcribe_post(location: &Location, id: &str) -> Result<()> {
    let temp_dir = location.temp_dir();
    if !temp_dir.exists() {
        fs::create_dir_all(&temp_dir)
            .with_context(|| "Failed to create temp directory for transcript file")?;
    }

    // "{temp_dir}/transcript.{id}"
    let mut temp_file_path = temp_dir.join("transcript");
    temp_file_path.set_extension(id);

    let completed_dir = location.posts_dir().join(id);

    let transcript_file_path = completed_dir.join(TRANSCRIPT_NAME);
    let esperanto_file_path = completed_dir.join(IMAGE_ESPERANTO_NAME);
    let english_file_path = completed_dir.join(IMAGE_ENGLISH_NAME);

    kill_process_class(IMAGE_CLASS_TRANSCRIBE)?;

    setup_image_viewer_window(&[esperanto_file_path, english_file_path])?;

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

    open_editor(&temp_file_path)?;

    kill_process_class(IMAGE_CLASS_TRANSCRIBE)?;

    if file_matches_string(&temp_file_path, &transcript_template)
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

fn is_id_sunday(id: &str) -> Result<bool> {
    let id_number = id
        .parse::<u32>()
        .with_context(|| "Post id is not an integer")?;
    let is_sunday = (id_number + 1) % 7 == 0;
    Ok(is_sunday)
}

fn file_matches_string(file_path: impl AsRef<Path>, target: &str) -> io::Result<bool> {
    // TODO(opt): This doesn't have to alloc a new String
    let file_contents = fs::read_to_string(file_path)?;
    Ok(file_contents == target)
}

fn revise_post(location: &Location, id: &str) -> Result<()> {
    let completed_dir = location.posts_dir();

    let date_file_path = completed_dir.join(id).join("date");
    let date_file = fs::read_to_string(date_file_path)?;
    let date = NaiveDate::parse_from_str(date_file.trim(), "%Y-%m-%d")
        .with_context(|| "Invalid date file for post")?;

    make_post(location, date, id, true).with_context(|| "Failed to make post")?;

    let post_path = completed_dir.join(id);
    let generated_path = location.generated_dir().join(id);

    // TODO(refactor): Inline closure manually
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
    wait_for_file(&post_path)?;

    Ok(())
}

fn print_confirmation(prompt: &str) {
    print!("{} ", prompt);
    io::stdout().flush().expect("failed to flush stdout");
    stdin_read_and_discard();
}

fn stdin_read_and_discard() {
    let mut reader = BufReader::new(io::stdin());
    let mut buf = [0];
    loop {
        reader.read_exact(&mut buf).expect("failed to read stdin");
        if buf[0] == b'\n' {
            return;
        }
    }
}

fn wait_for_file(path: impl AsRef<Path>) -> Result<()> {
    const WAIT_DELAY: Duration = Duration::from_millis(500);
    while !path.as_ref().exists() {
        thread::sleep(WAIT_DELAY);
    }
    Ok(())
}

fn get_revise_id(location: &Location, id: Option<String>) -> Result<String> {
    if let Some(id) = id {
        if !post_exists(location, &id) {
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

fn get_transcribe_id(location: &Location, id: Option<String>) -> Result<String> {
    if let Some(id) = id {
        if !post_exists(location, &id) {
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

fn post_exists(location: &Location, id: &str) -> bool {
    location.posts_dir().join(id).is_dir()
}

fn find_unrevised_post(location: &Location) -> Result<Option<String>> {
    let completed_dir = location.posts_dir();

    if let Some(id) = find_post(&completed_dir, |path| {
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

    if let Some(id) = find_post(&completed_dir, |path| {
        let svg_file_path = path.join(IMAGE_SVG_NAME);
        Ok(!svg_file_path.exists())
    })? {
        return Ok(Some(id));
    }

    Ok(None)
}

fn find_untranscribed_post(location: &Location) -> Result<Option<String>> {
    let completed_dir = location.posts_dir();

    if let Some(id) = find_post(&completed_dir, |path| {
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
    location: &Location,
    date: NaiveDate,
    name: &str,
    skip_post_check: bool,
) -> Result<()> {
    let generated_dir = location.generated_dir();

    let mut original_comic_path = location.source_dir().join(date.to_string());
    original_comic_path.set_extension(ORIGINAL_COMIC_FORMAT);
    let output_dir = generated_dir.join(name);
    // TODO(refactor): Define file names as constants
    let title_file_path = output_dir.join(TITLE_NAME);
    let date_file_path = output_dir.join(DATE_NAME);
    let generated_comic_path = output_dir.join(IMAGE_ENGLISH_NAME);
    let duplicate_comic_path = output_dir.join(IMAGE_ESPERANTO_NAME);

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
        .save(&generated_comic_path)
        .with_context(|| "Failed to save generated image")?;

    fs::copy(&generated_comic_path, &duplicate_comic_path)
        .with_context(|| "Failed to copy generated image")?;

    println!("Created {}", name);

    Ok(())
}

fn get_random_watermark(location: &Location) -> Result<String> {
    let contents =
        fs::read_to_string(location.watermarks_file()).with_context(|| "Failed to read file")?;
    let watermarks: Vec<&str> = contents.lines().collect();
    let index = random::with_rng(|rng| rng.gen_range(0..watermarks.len()));
    Ok(watermarks[index].to_string())
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

fn get_date(location: &Location, date: Option<NaiveDate>, recent: bool) -> Result<NaiveDate> {
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

fn get_recent_date(location: &Location) -> Result<NaiveDate> {
    let recent_file = location.recent_file();

    if !recent_file.exists() {
        bail!("Cache file does not yet exist");
    }
    let file = fs::OpenOptions::new().read(true).open(&recent_file)?;
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
                Err(error) => bail!("Cache file contains invalid date: {}", error),
            }
        }
    }
}

fn show_comic(location: &Location, date: Option<NaiveDate>) -> Result<()> {
    let source_dir = location.source_dir();

    let (date, path) = match date {
        Some(date) => {
            let mut path = source_dir.join(date.to_string());
            path.set_extension(ORIGINAL_COMIC_FORMAT);
            (date, path)
        }
        None => {
            // TODO(fix): check if length == 0
            let path = get_random_directory_entry(&source_dir)
                .with_context(|| "Failed to read comics directory")?
                .path();
            let date = get_date_from_path(&path).with_context(|| {
                "Found comic file with invalid name. Should contain date in YYYY-MM-DD format."
            })?;
            (date, path)
        }
    };

    println!("{}", date);

    append_recent_date(location, date).with_context(|| "Failed to append to cache file")?;

    kill_process_class(IMAGE_CLASS_SHOW)?;
    spawn_image_viewer(&[path], IMAGE_CLASS_SHOW, true)?;

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

fn append_recent_date(location: &Location, date: NaiveDate) -> io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(location.recent_file())?;
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
