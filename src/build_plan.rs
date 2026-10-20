use std::sync::mpsc;

use crate::command::run_command_with_logs;
use crate::config::SunixConfig;
use crate::dix::nix_build_command;

#[derive(Debug, Default)]
pub(crate) struct BuildPlan {
    pub builds: Vec<String>,
    pub downloads: Vec<String>,
    pub unknown: Vec<String>,
    pub download_summary: Option<String>,
    pub output: String,
}

pub(crate) fn build_plan(
    config: &SunixConfig,
    home_manager: bool,
    logs: mpsc::Sender<String>,
) -> Result<BuildPlan, String> {
    let (directory, attr) = if home_manager {
        (
            config.home_flake_dir(),
            format!(
                ".#homeConfigurations.{}.activationPackage",
                config.home_flake
            ),
        )
    } else {
        (
            config.nixos_flake_dir(),
            format!(
                ".#nixosConfigurations.{}.config.system.build.toplevel",
                config.nixos_flake
            ),
        )
    };
    let mut command = nix_build_command(config, &attr, directory);
    command
        .arg("--dry-run")
        .arg("--log-format")
        .arg("raw")
        .env("LC_ALL", "C")
        .env("NO_COLOR", "1");
    let output = run_command_with_logs(
        &mut command,
        &format!("nix build --dry-run {attr} in {}", directory.display()),
        Some(&logs),
    )?;
    Ok(parse_build_plan(&String::from_utf8_lossy(&output.stderr)))
}

fn parse_build_plan(output: &str) -> BuildPlan {
    let mut plan = BuildPlan {
        output: output.trim().to_owned(),
        ..BuildPlan::default()
    };
    enum Section {
        None,
        Build,
        Download,
        Unknown,
    }
    let mut section = Section::None;
    for line in output.lines() {
        let text = line.trim();
        if (text.starts_with("this derivation ") || text.starts_with("these "))
            && text.ends_with("will be built:")
        {
            section = Section::Build;
        } else if (text.starts_with("this path ") || text.starts_with("these "))
            && text.contains("will be fetched (")
            && text.ends_with("):")
        {
            section = Section::Download;
            plan.download_summary = text
                .split_once("will be fetched (")
                .map(|(_, sizes)| sizes.trim_end_matches("):").to_owned());
        } else if text.starts_with("don't know how to build these paths") {
            section = Section::Unknown;
        } else if line.starts_with("  ") && text.starts_with("/nix/store/") {
            match section {
                Section::Build => plan.builds.push(text.to_owned()),
                Section::Download => plan.downloads.push(text.to_owned()),
                Section::Unknown => plan.unknown.push(text.to_owned()),
                Section::None => {}
            }
        } else if !text.is_empty() {
            section = Section::None;
        }
    }
    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_builds_downloads_and_sizes_without_mistaking_warnings_for_paths() {
        let plan = parse_build_plan(
            "warning: flake is dirty\nthese 2 derivations will be built:\n  /nix/store/aaa-system.drv\n  /nix/store/bbb-home.drv\nthis path will be fetched (1.20 MiB download, 4.00 MiB unpacked):\n  /nix/store/ccc-hello-2.12\nwarning: something else\n  /nix/store/ddd-unrelated\n",
        );
        assert_eq!(plan.builds.len(), 2);
        assert_eq!(plan.downloads, ["/nix/store/ccc-hello-2.12"]);
        assert_eq!(
            plan.download_summary.as_deref(),
            Some("1.20 MiB download, 4.00 MiB unpacked")
        );
        assert!(plan.output.contains("flake is dirty"));
    }

    #[test]
    fn handles_singular_build_plural_downloads_and_unknown_paths() {
        let plan = parse_build_plan(
            "this derivation will be built:\n  /nix/store/a-build.drv\nthese 2 paths will be fetched (2 MiB download, 6 MiB unpacked):\n  /nix/store/b-one\n  /nix/store/c-two\ndon't know how to build these paths (may be caused by read-only store access):\n  /nix/store/d-missing\n",
        );
        assert_eq!(plan.builds.len(), 1);
        assert_eq!(plan.downloads.len(), 2);
        assert_eq!(plan.unknown, ["/nix/store/d-missing"]);
    }

    #[test]
    fn preserves_unrecognized_output_and_handles_no_work() {
        let plan = parse_build_plan("");
        assert!(plan.builds.is_empty() && plan.downloads.is_empty() && plan.output.is_empty());
        let plan = parse_build_plan("an unfamiliar Nix message");
        assert_eq!(plan.output, "an unfamiliar Nix message");
    }
}
