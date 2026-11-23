//! `yukti completions <shell>`: prints a completion script. No network.
//!
//! ```text
//! yukti completions zsh  > ~/.zfunc/_yukti
//! yukti completions bash > /usr/local/etc/bash_completion.d/yukti
//! yukti completions fish > ~/.config/fish/completions/yukti.fish
//! ```

use clap::CommandFactory;

use crate::cli::{Cli, CompletionsArgs};
use crate::error::exit;
use crate::output::Rendered;

pub fn run(args: &CompletionsArgs) -> Rendered {
    let mut command = Cli::command();
    let mut buf = Vec::new();
    clap_complete::generate(args.shell, &mut command, "yukti", &mut buf);
    Rendered::stdout(String::from_utf8_lossy(&buf).into_owned(), exit::SUCCESS)
}
