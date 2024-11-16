pub mod actions;
pub mod names;

mod commands;
mod file;
mod location;
mod random;

pub use location::Location;
pub use random::init_rng;

mod constants {
    pub const SOURCE_FORMAT: &str = "png";
    pub mod post_file {
        pub const INITIAL: &str = "esperanto.png";
        pub const DUPLICATE: &str = "english.png";
        pub const SVG: &str = "esperanto.svg";
        pub const TITLE: &str = "title";
        pub const DATE: &str = "date";
        pub const TRANSCRIPT: &str = "transcript";
        pub const PROPS: &str = "props";
        pub const SPECIAL: &str = "special";
    }
    pub mod viewer_class {
        pub const TRANSCRIBE: &str = "garfutils-transcribe";
        pub const SHOW: &str = "garfutils-show";
    }
}

pub fn confirm(prompt: &str) {
    use std::io::{self, Write as _};
    print!("{} ", prompt);
    io::stdout().flush().expect("failed to flush stdout");
    file::discard_read_line(&mut io::stdin());
}
