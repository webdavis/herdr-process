mod arguments;
mod attach;
mod controller;
mod environment;
mod manager;

use anyhow::{Context, Result};
use arguments::Arguments;
use herdr_process_adapters as adapters;

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
    // The fresh guardian must dispatch before any background thread exists.
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
        Arguments::Help => println!(
            "herdr-process\n\n  generate --output DIRECTORY [--profiles FILE] [--herdr-config FILE]\n  action PROFILE split-right|split-below|toggle-float|kill [--profiles FILE] [--herdr-config FILE]"
        ),
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
