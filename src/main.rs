mod args;

use std::fs::{self, DirEntry};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::NaiveDate;
use clap::Parser;
use rand::Rng;

fn main() {
    let args = args::Args::parse();

    let dir_config = DirConfig {
        original_comics_dir: PathBuf::from("/home/darcy/pics/garfield"),
        recently_shown_file: PathBuf::from("/home/darcy/.cache/garfutils.recent"),
    };

    match args.command {
        args::Command::Show { date } => {
            show_comic(&dir_config, date);
        }

        args::Command::Make {
            date,
            recent,
            name,
            skip_check,
        } => {
            let date = if recent {
                assert!(
                    date.is_none(),
                    "date should be `None` with `--recent` (cli parsing is broken)"
                );
                // TODO(feat/error): Handle better
                match get_recent_date(&dir_config).unwrap() {
                    Some(date) => date,
                    None => {
                        eprintln!("no recent comic.");
                        return;
                    }
                }
            } else {
                date.expect("date should be `Some` without `--recent` (cli parsing is broken)")
            };
            println!("date: {}", date);
            todo!("make");
        }

        args::Command::Revise { .. } => todo!(),

        args::Command::Transcribe { .. } => todo!(),
    }
}

struct DirConfig {
    pub original_comics_dir: PathBuf,
    pub recently_shown_file: PathBuf,
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
