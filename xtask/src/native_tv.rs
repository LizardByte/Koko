//! Parse explicit native TV preparation options for offline package builders.

use crate::live_tv::{
    self,
    BuildOptions,
};
use std::{
    error::Error,
    path::Path,
};

fn usage() {
    println!(
        "Usage: cargo xtask prepare-live-tv --target <triple> --destination <directory>\nOptions: \
         --cache <directory> --source <checkout> --revision <id> --jobs <count> \
         --system-cpp-runtime"
    );
}

pub(super) fn run(
    args: Vec<String>,
    workspace: &Path,
) -> Result<(), Box<dyn Error>> {
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        usage();
        return Ok(());
    }
    live_tv::build(workspace, &parse(args)?)
}

fn parse(args: Vec<String>) -> Result<BuildOptions, Box<dyn Error>> {
    let mut options = BuildOptions::new(String::new(), Default::default());
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if arg == "--system-cpp-runtime" {
            options.system_cpp_runtime = true;
            continue;
        }
        let (name, inline) = arg
            .split_once('=')
            .map_or((arg.as_str(), None), |(name, value)| {
                (name, Some(value.to_owned()))
            });
        let value = inline
            .or_else(|| args.next())
            .ok_or_else(|| format!("missing value after {name}"))?;
        match name {
            "--target" => options.target = value,
            "--destination" => options.destination = value.into(),
            "--cache" => options.cache = value.into(),
            "--source" => options.source = Some(value.into()),
            "--revision" => options.revision = Some(value),
            "--jobs" => {
                options.jobs = value
                    .parse()
                    .map_err(|_| "jobs must be a positive integer")?
            }
            _ => return Err(format!("unknown native TV option `{name}`").into()),
        }
    }
    if options.target.is_empty() || options.destination.as_os_str().is_empty() {
        return Err("--target and --destination are required".into());
    }
    if options.jobs == 0 {
        return Err("jobs must be a positive integer".into());
    }
    Ok(options)
}

#[cfg(test)]
mod tests {
    use super::parse;
    use std::path::Path;

    fn arguments(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn keeps_paths_and_offline_build_options_intact() {
        let options = parse(arguments(&[
            "--target=aarch64-unknown-linux-gnu",
            "--destination",
            "/app/native tv",
            "--cache",
            "cache folder",
            "--source",
            "source folder",
            "--revision",
            "abc-123",
            "--jobs",
            "4",
            "--system-cpp-runtime",
        ]))
        .unwrap();
        assert_eq!(options.target, "aarch64-unknown-linux-gnu");
        assert_eq!(options.destination, Path::new("/app/native tv"));
        assert_eq!(options.cache, Path::new("cache folder"));
        assert_eq!(options.source.as_deref(), Some(Path::new("source folder")));
        assert_eq!(options.revision.as_deref(), Some("abc-123"));
        assert_eq!(options.jobs, 4);
        assert!(options.system_cpp_runtime);
    }

    #[test]
    fn rejects_missing_or_invalid_options_before_building() {
        for extra in [
            &["--jobs", "0"][..],
            &["--jobs", "-1"],
            &["--jobs", "abc"],
            &["--cache"],
            &["--unexpected", "value"],
        ] {
            let mut args = arguments(&[
                "--target",
                "x86_64-pc-windows-msvc",
                "--destination",
                "output",
            ]);
            args.extend(arguments(extra));
            assert!(parse(args).is_err());
        }
        assert!(parse(Vec::new()).is_err());
        assert!(
            parse(arguments(&[
                "--target",
                "x86_64-pc-windows-msvc"
            ]))
            .is_err()
        );
    }
}
