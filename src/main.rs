mod args;

use anyhow::{Context, Result};
use clap::Parser;

use garfutils::{actions, names, confirm, Location};

fn main() -> Result<()> {
    garfutils::init_rng();
    let args = args::Args::parse();
    let location = Location::from(args.location).with_context(|| "Failed to verify location")?;

    match args.command {
        args::Command::Show { date } => {
            actions::show_comic(&location, date)?;
        }

        args::Command::Make { date, recent } => {
            let date =
                names::get_date(&location, date, recent).with_context(|| "Failed to get date")?;
            let name = names::get_unique_name(date);
            actions::make_post(&location, date, &name, false)
                .with_context(|| "Failed to make post")?;
        }

        args::Command::Revise { id } => {
            let id = names::get_revise_id(&location, id)?;
            actions::revise_post(&location, &id).with_context(|| "Failed to revise post")?;
            confirm("Transcribe now?");
            actions::transcribe_post(&location, &id)
                .with_context(|| "Failed to transcribe post")?;
        }

        args::Command::Transcribe { id } => {
            let id = names::get_transcribe_id(&location, id)?;
            actions::transcribe_post(&location, &id)
                .with_context(|| "Failed to transcribe post")?;
        }
    }

    Ok(())
}
