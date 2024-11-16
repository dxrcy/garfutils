use std::path::PathBuf;

use anyhow::{bail, Result};

pub struct Location {
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

    pub fn from(base_dir: Option<PathBuf>) -> Result<Self> {
        let base_dir = Self::get_base_dir(base_dir)?;
        let location = Self { base_dir };
        location.check_dirs_exist()?;
        Ok(location)
    }

    fn get_base_dir(base_dir: Option<PathBuf>) -> Result<PathBuf> {
        if let Some(path) = base_dir {
            return Ok(path);
        }
        if let Some(path) = dirs_next::data_dir() {
            return Ok(path.join(Self::DEFAULT_LOCATION_NAME));
        }
        bail!(
            "Reading standard data location.\n\
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

        let expected_sub_dirs: &[(fn(_) -> _, _, _)] = &[
            (Self::source_dir, Self::SOURCE_DIR, true),
            (Self::generated_dir, Self::GENERATED_DIR, true),
            (Self::posts_dir, Self::POSTS_DIR, true),
            (Self::old_dir, Self::OLD_DIR, true),
            (Self::watermarks_file, Self::WATERMARKS_FILE, false),
            (Self::icon_file, Self::ICON_FILE, false),
        ];
        for (path_fn, name, is_dir) in expected_sub_dirs {
            let path = path_fn(self);
            let is_correct_kind = if *is_dir {
                path.is_dir()
            } else {
                path.is_file()
            };
            if !is_correct_kind {
                bail!(
                    "Location is missing {}: `{}`\n{}",
                    if *is_dir { "sub-directory" } else { "file" },
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
