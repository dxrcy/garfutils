mod args;

use std::fs::{self, DirEntry};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::NaiveDate;
use clap::Parser;
use rand::Rng;

fn main() {
    let args = args::Args::parse();

    let dir_config = DirConfig {
        original_comics: PathBuf::from("/home/darcy/pics/garfield"),
    };

    match args.command {
        args::Command::Show { date } => {
            show_comic(&dir_config, date);
        }

        args::Command::Make { .. } => todo!(),

        args::Command::Revise { .. } => todo!(),

        args::Command::Transcribe { .. } => todo!(),
    }
}

struct DirConfig {
    pub original_comics: PathBuf,
}

macro_rules! command {
    (
        $name:expr, $( $arg:expr ),* $(,)?
    ) => {{
        Command::new($name)
            $( .arg($arg) )*
    }};
}

fn show_comic(dir_config: &DirConfig, date: Option<NaiveDate>) {
    let mut rng = rand::thread_rng();

    let path = match date {
        Some(date) => dir_config.original_comics.join(date.to_string() + ".png"),
        None => get_random_directory_entry(&mut rng, &dir_config.original_comics)
            .unwrap()
            .path(),
    };

    println!("{:?}", path);

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
    .unwrap();
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
