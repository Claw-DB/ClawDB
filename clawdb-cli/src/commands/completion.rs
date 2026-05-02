//! `clawdb completion` — generate shell completion scripts.

use clap::Args;
use clap_complete::Shell;

#[derive(Debug, Clone, Args)]
pub struct CompletionArgs {
    /// Target shell.
    pub shell: Shell,
}

pub fn execute(args: CompletionArgs, app: &mut clap::Command) {
    let bin_name = app.get_name().to_string();
    clap_complete::generate(args.shell, app, bin_name, &mut std::io::stdout());

    eprintln!();
    match args.shell {
        Shell::Bash => {
            eprintln!("# To install, add this to ~/.bashrc:");
            eprintln!("# eval \"$(clawdb completion bash)\"");
        }
        Shell::Zsh => {
            eprintln!("# To install, add this to ~/.zshrc:");
            eprintln!("# eval \"$(clawdb completion zsh)\"");
        }
        Shell::Fish => {
            eprintln!("# To install, run:");
            eprintln!("# clawdb completion fish | source");
        }
        Shell::PowerShell => {
            eprintln!("# To install, add this to your $PROFILE:");
            eprintln!("# Invoke-Expression (& clawdb completion powershell | Out-String)");
        }
        Shell::Elvish => {
            eprintln!("# To install, pipe to a file and source it in rc.elv");
        }
        _ => {}
    }
}
