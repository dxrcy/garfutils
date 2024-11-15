use std::path::PathBuf;

use chrono::NaiveDate;
use clap::{ArgGroup, Parser, Subcommand};

/// GarfUtils
///
/// A set of utilities for translating Garfield comics
#[derive(Debug, Parser)]
#[clap(author, version, about)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
    /// Parent directory of input and output directories
    ///
    /// Default: `$XDG_DATA_HOME/garfutils` or `$HOME/.local/share/garfutils`
    ///
    /// Expects sub-directories `comics`, `generated`, `completed`, each of which may be symlinks
    #[arg(long)]
    pub location: Option<PathBuf>,
    /// Cache file path
    ///
    /// Default: `$XDG_CONFIG_HOME/garfutils.recent` or `$HOME/.cache/garfutils.recent`
    #[arg(long)]
    pub cache_file: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Display an original comic, given a date
    #[clap(alias = "s")]
    Show {
        /// Date of the comic to display (defaults to a random date)
        date: Option<NaiveDate>,
    },

    /// Create a new post, given a date
    #[clap(alias = "m")]
    #[clap(group(ArgGroup::new("date_group").required(true)))]
    Make {
        /// Date of the comic to create into a post
        #[arg(group("date_group"))]
        date: Option<NaiveDate>,
        /// Use most recently displayed comic `show` instead of specifying a date
        #[arg(short, long, group("date_group"))]
        recent: bool,
    },

    /// Recreate an existing post, given an id
    #[clap(alias = "r")]
    Revise {
        /// Id of the post to recreate
        id: Option<String>,
    },

    /// Transcribe an existing post, given an id
    ///
    /// Displays post, and opens editor to input transcription
    #[clap(alias = "t")]
    Transcribe {
        /// Id of the post to transcribe
        id: Option<String>,
    },
}
