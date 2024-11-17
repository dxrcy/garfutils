mod args;

use anyhow::{Context, Result};
use clap::Parser;

use garfutils::{actions, confirm, names, Location};

fn main() -> Result<()> {
    garfutils::init_rng();
    let args = args::Args::parse();
    let location = Location::from(args.location).with_context(|| "Parsing directory location")?;

    match args.command {
        args::Command::Show { date } => {
            actions::show(&location, date).with_context(|| "Showing comic")?;
        }

        args::Command::Make { date, recent } => {
            let date = names::get_date(&location, date, recent).with_context(|| "Parsing date")?;
            let name = names::generate_name(date);
            actions::make(&location, date, &name, false).with_context(|| "Generating post")?;
        }

        args::Command::Transcribe { id } => {
            let id = names::get_transcribe_id(&location, id).with_context(|| "Parsing post id")?;
            actions::transcribe(&location, &id).with_context(|| "Transcribing post")?;
        }

        args::Command::Revise { id } => {
            let id = names::get_revise_id(&location, id).with_context(|| "Parsing post id")?;
            let date = names::read_date(&location, &id)
                .with_context(|| "Reading date from existing post directory")?;
            actions::make(&location, date, &id, true).with_context(|| "Generating post")?;
            actions::revise(&location, &id).with_context(|| "Revising post")?;
            confirm("Transcribe now?");
            actions::transcribe(&location, &id).with_context(|| "Transcribing post")?;
        }
    }

    Ok(())
}
