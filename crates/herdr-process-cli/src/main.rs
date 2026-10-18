mod arguments;
mod attach;
mod controller;
mod environment;
mod manager;

use anyhow::{Context, Result};
use arguments::Arguments;
use herdr_process_adapters as adapters;

const HELP: &str = "\
herdr-process keeps one process alive per profile and shows it in herdr panes.

Usage:
  herdr-process generate --output DIRECTORY [--profiles FILE] [--herdr-config FILE]
      Write herdr-plugin.toml to DIRECTORY with four actions for each profile.
  herdr-process action PROFILE ACTION [--profiles FILE] [--herdr-config FILE]
      Run ACTION for PROFILE: split-right, split-below, toggle-float or kill.

Options:
  --profiles FILE      Profiles to read. Default: processes.toml next to the herdr config.
  --herdr-config FILE  herdr config to read. Default: $HERDR_CONFIG_PATH, else
                       $XDG_CONFIG_HOME/herdr/config.toml, else ~/.config/herdr/config.toml.
";

fn main() {
    let parsed = std::env::args_os()
        .skip(1)
        .map(|arg| {
            arg.into_string()
                .map_err(|_| anyhow::anyhow!("arguments must be valid UTF-8"))
        })
        .collect::<Result<Vec<_>>>()
        .and_then(|args| arguments::parse(&args));
    let arguments = match parsed {
        Ok(arguments) => arguments,
        Err(error) => {
            eprintln!("herdr-process: {error}");
            std::process::exit(2);
        }
    };
    let result = match arguments {
        Arguments::Supervise(remaining) => adapters::supervise(&remaining),
        other => dispatch(other).map(|()| 0),
    };
    match result {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("herdr-process: {error:#}");
            std::process::exit(1);
        }
    }
}

fn dispatch(arguments: Arguments) -> Result<()> {
    match arguments {
        Arguments::Help => print!("{HELP}"),
        Arguments::Generate { options, output } => {
            environment::Environment::read(&options, |name| std::env::var_os(name))?
                .load()?
                .write_manifest(&output)?;
        }
        Arguments::Action {
            options,
            profile,
            action,
        } => controller::run(&options, &profile, action)?,
        Arguments::Attach => attach::run()?,
        Arguments::Manager { options, runtime } => {
            let environment =
                environment::Environment::read(&options, |name| std::env::var_os(name))?;
            let endpoint = adapters::Endpoint::new(&runtime)?;
            if let Some(listener) = endpoint.bind()? {
                let configuration = environment.load()?;
                let host = environment::host(|name| std::env::var_os(name))?;
                controller::manager_started("ready");
                let executable =
                    std::env::current_exe().context("cannot locate process supervisor")?;
                manager::run(listener, configuration, host, executable, endpoint.socket())?;
            } else {
                controller::manager_started("duplicate");
            }
        }
        Arguments::Supervise(_) => unreachable!("supervise dispatched before composition"),
    }
    Ok(())
}
