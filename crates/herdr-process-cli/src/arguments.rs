use anyhow::{Result, bail, ensure};
use herdr_process_domain::Action;
use std::path::PathBuf;

#[derive(Debug, Default, PartialEq)]
pub struct Options {
    pub profiles: Option<PathBuf>,
    pub herdr: Option<PathBuf>,
}

#[derive(Debug, PartialEq)]
pub enum Arguments {
    Generate {
        options: Options,
        output: PathBuf,
    },
    Action {
        options: Options,
        profile: String,
        action: Action,
    },
    Manager {
        options: Options,
        runtime: PathBuf,
    },
    Attach,
    Supervise(Vec<String>),
    Help,
}

pub fn parse(arguments: &[String]) -> Result<Arguments> {
    let Some(command) = arguments.first().map(String::as_str) else {
        return Ok(Arguments::Help);
    };
    let rest = &arguments[1..];
    if (command == "--help" && rest.is_empty())
        || (matches!(command, "action" | "generate" | "attach") && rest == ["--help"])
    {
        return Ok(Arguments::Help);
    }
    match command {
        "attach" if rest.is_empty() => Ok(Arguments::Attach),
        "supervise" if rest.len() == 2 => Ok(Arguments::Supervise(rest.to_vec())),
        "action" if rest.len() >= 2 => {
            let action = rest[1].parse()?;
            let (options, _) = parse_options(&rest[2..], None)?;
            Ok(Arguments::Action {
                options,
                profile: rest[0].clone(),
                action,
            })
        }
        "generate" => {
            let (options, output) = parse_options(rest, Some("--output"))?;
            Ok(Arguments::Generate {
                options,
                output: output
                    .ok_or_else(|| anyhow::anyhow!("generate requires --output DIRECTORY"))?,
            })
        }
        "manager" => {
            let (options, runtime) = parse_options(rest, Some("--runtime-dir"))?;
            Ok(Arguments::Manager {
                options,
                runtime: runtime
                    .ok_or_else(|| anyhow::anyhow!("manager requires --runtime-dir DIRECTORY"))?,
            })
        }
        _ => bail!("invalid command or arguments; see --help"),
    }
}

fn parse_options(
    arguments: &[String],
    destination_flag: Option<&str>,
) -> Result<(Options, Option<PathBuf>)> {
    ensure!(
        arguments.len().is_multiple_of(2),
        "each option requires a value"
    );
    let mut options = Options::default();
    let mut destination = None;
    for pair in arguments.as_chunks::<2>().0 {
        let flag = pair[0].as_str();
        let slot = match flag {
            "--profiles" => &mut options.profiles,
            "--herdr-config" => &mut options.herdr,
            _ if Some(flag) == destination_flag => &mut destination,
            _ => bail!("unknown option {flag}"),
        };
        ensure!(slot.is_none(), "duplicate option {flag}");
        *slot = Some(PathBuf::from(&pair[1]));
    }
    Ok((options, destination))
}

#[cfg(test)]
mod tests;
