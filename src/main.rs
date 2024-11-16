mod args;

use anyhow::{Context, Result};
use clap::Parser;

use garfutils::*;

fn main() -> Result<()> {
    garfutils::init_rng();
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
