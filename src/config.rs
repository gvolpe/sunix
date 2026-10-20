use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct SunixConfig {
    pub flake_dir: PathBuf,
    pub home_flake_dir: Option<PathBuf>,
    pub nixos_flake_dir: Option<PathBuf>,
    pub home_flake: String,
    pub nixos_flake: String,
    pub nixpkgs_input: String,
    pub nixpkgs_url: String,
    pub dix_binary: Option<PathBuf>,
    pub style_css: Option<PathBuf>,
    pub show_demo: bool,
    pub enable_build_plans: bool,
}

impl SunixConfig {
    pub fn home_flake_dir(&self) -> &Path {
        self.home_flake_dir.as_deref().unwrap_or(&self.flake_dir)
    }

    pub fn nixos_flake_dir(&self) -> &Path {
        self.nixos_flake_dir.as_deref().unwrap_or(&self.flake_dir)
    }
}

pub fn load_config() -> Result<SunixConfig, String> {
    let path = config_path()?;
    let content = fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;

    parse_config(&content, &path)
}

fn config_path() -> Result<PathBuf, String> {
    let config_home = env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            env::var_os("HOME")
                .filter(|value| !value.is_empty())
                .map(|home| PathBuf::from(home).join(".config"))
        })
        .ok_or_else(|| {
            "XDG_CONFIG_HOME is unset and HOME is unset; cannot locate sunix config".to_owned()
        })?;

    Ok(config_home.join("sunix").join("sunix.toml"))
}

fn parse_config(content: &str, source: &Path) -> Result<SunixConfig, String> {
    let mut values = HashMap::new();

    for (index, line) in content.lines().enumerate() {
        let line = strip_comment(line).trim();
        if line.is_empty() {
            continue;
        }

        let (key, value) = line.split_once('=').ok_or_else(|| {
            format!(
                "failed to parse {}:{}: expected key=value",
                source.display(),
                index + 1
            )
        })?;
        let key = key.trim();
        let value = parse_value(value.trim(), source, index + 1)?;

        values.insert(key.to_owned(), value);
    }

    let flake_dir = required_field(&values, "flake_dir", source)?;
    let home_flake_dir = optional_path_field(&values, "home_flake_dir");
    let nixos_flake_dir = optional_path_field(&values, "nixos_flake_dir");
    let home_flake = required_field(&values, "home_flake", source)?;
    let nixos_flake = required_field(&values, "nixos_flake", source)?;
    let nixpkgs_input = default_string_field(&values, "nixpkgs_input", "nixpkgs");
    let nixpkgs_url = default_string_field(
        &values,
        "nixpkgs_url",
        "https://channels.nixos.org/nixos-unstable/nixexprs.tar.zst",
    );
    let dix_binary = optional_path_field(&values, "dix_binary");
    let style_css = optional_path_field(&values, "style_css");
    let show_demo = optional_bool_field(&values, "show_demo", source)?;
    let enable_build_plans = optional_bool_field(&values, "enable_build_plans", source)?;

    Ok(SunixConfig {
        flake_dir: expand_home_path(&flake_dir),
        home_flake_dir,
        nixos_flake_dir,
        home_flake,
        nixos_flake,
        nixpkgs_input,
        nixpkgs_url,
        dix_binary,
        style_css,
        show_demo,
        enable_build_plans,
    })
}

fn required_field(
    values: &HashMap<String, String>,
    field: &str,
    source: &Path,
) -> Result<String, String> {
    match values.get(field).map(|value| value.trim()) {
        Some(value) if !value.is_empty() => Ok(value.to_owned()),
        Some(_) => Err(format!(
            "{} has an empty mandatory `{field}` field",
            source.display()
        )),
        None => Err(format!(
            "{} is missing mandatory `{field}` field",
            source.display()
        )),
    }
}

fn default_string_field(values: &HashMap<String, String>, field: &str, default: &str) -> String {
    values
        .get(field)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or(default)
        .to_owned()
}

fn optional_path_field(values: &HashMap<String, String>, field: &str) -> Option<PathBuf> {
    values
        .get(field)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(expand_home_path)
}

fn optional_bool_field(
    values: &HashMap<String, String>,
    field: &str,
    source: &Path,
) -> Result<bool, String> {
    match values.get(field).map(|value| value.trim()) {
        None | Some("") | Some("false") => Ok(false),
        Some("true") => Ok(true),
        Some(value) => Err(format!(
            "{} has invalid optional `{field}` value `{value}`; expected true or false",
            source.display()
        )),
    }
}

fn parse_value(value: &str, source: &Path, line: usize) -> Result<String, String> {
    if let Some(quote) = value
        .chars()
        .next()
        .filter(|char| *char == '"' || *char == '\'')
    {
        if !value.ends_with(quote) || value.len() < 2 {
            return Err(format!(
                "failed to parse {}:{line}: unterminated quoted value",
                source.display()
            ));
        }

        return Ok(value[1..value.len() - 1].to_owned());
    }

    Ok(value.to_owned())
}

fn strip_comment(line: &str) -> &str {
    let mut quote = None;

    for (index, char) in line.char_indices() {
        match (quote, char) {
            (Some(active), current) if active == current => quote = None,
            (None, '"' | '\'') => quote = Some(char),
            (None, '#') => return &line[..index],
            _ => {}
        }
    }

    line
}

fn expand_home_path(value: &str) -> PathBuf {
    let Some(home) = env::var_os("HOME").filter(|value| !value.is_empty()) else {
        return PathBuf::from(value);
    };

    if value == "$HOME" || value == "${HOME}" || value == "~" {
        return PathBuf::from(home);
    }

    if let Some(rest) = value.strip_prefix("$HOME/") {
        return PathBuf::from(home).join(rest);
    }

    if let Some(rest) = value.strip_prefix("${HOME}/") {
        return PathBuf::from(home).join(rest);
    }

    if let Some(rest) = value.strip_prefix("~/") {
        return PathBuf::from(home).join(rest);
    }

    PathBuf::from(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_build_plan_opt_in() {
        for (setting, expected) in [
            ("", false),
            ("enable_build_plans=", false),
            ("enable_build_plans=false", false),
            ("enable_build_plans=true", true),
            ("enable_build_plans='true' # enabled", true),
        ] {
            let config = parse_config(
                &format!("flake_dir=/tmp/flake\nhome_flake=home\nnixos_flake=host\n{setting}\n"),
                Path::new("sunix.toml"),
            )
            .unwrap();
            assert_eq!(config.enable_build_plans, expected, "{setting}");
        }
    }

    #[test]
    fn rejects_invalid_build_plan_opt_in() {
        let error = parse_config(
            "flake_dir=/tmp/flake\nhome_flake=home\nnixos_flake=host\nenable_build_plans=yes\n",
            Path::new("sunix.toml"),
        )
        .unwrap_err();
        assert!(error.contains("invalid optional `enable_build_plans` value `yes`"));
    }

    #[test]
    fn defaults_nixpkgs_override_when_missing_or_empty() {
        for optional_fields in ["", "nixpkgs_input=\nnixpkgs_url=\n"] {
            let config = parse_config(
                &format!(
                    "flake_dir=/tmp/flake\nhome_flake=home\nnixos_flake=host\n{optional_fields}"
                ),
                Path::new("sunix.toml"),
            )
            .unwrap();

            assert_eq!(config.nixpkgs_input, "nixpkgs");
            assert_eq!(
                config.nixpkgs_url,
                "https://channels.nixos.org/nixos-unstable/nixexprs.tar.zst"
            );
        }
    }

    #[test]
    fn parses_custom_nixpkgs_override() {
        let config = parse_config(
            "flake_dir=/tmp/flake\nhome_flake=home\nnixos_flake=host\nnixpkgs_input='nixpkgs-unstable'\nnixpkgs_url=\"github:NixOS/nixpkgs/nixos-unstable\"\n",
            Path::new("sunix.toml"),
        )
        .unwrap();

        assert_eq!(config.nixpkgs_input, "nixpkgs-unstable");
        assert_eq!(config.nixpkgs_url, "github:NixOS/nixpkgs/nixos-unstable");
    }

    #[test]
    fn parses_required_fields_from_bare_values() {
        let config = parse_config(
            "\
flake_dir=$HOME/workspace/nix-config
home_flake=niri-hdmi
nixos_flake=aorus
",
            Path::new("sunix.toml"),
        )
        .unwrap();

        assert_eq!(
            config.flake_dir,
            expand_home_path("$HOME/workspace/nix-config")
        );
        assert_eq!(config.home_flake, "niri-hdmi");
        assert_eq!(config.nixos_flake, "aorus");
        assert_eq!(config.home_flake_dir, None);
        assert_eq!(config.nixos_flake_dir, None);
        assert_eq!(config.home_flake_dir(), config.flake_dir.as_path());
        assert_eq!(config.nixos_flake_dir(), config.flake_dir.as_path());
        assert_eq!(config.dix_binary, None);
        assert_eq!(config.style_css, None);
        assert!(!config.show_demo);
    }

    #[test]
    fn parses_required_fields_from_quoted_values() {
        let config = parse_config(
            "\
flake_dir = \"~/workspace/nix-config\"
home_flake = \"niri-hdmi\"
nixos_flake = 'aorus'
",
            Path::new("sunix.toml"),
        )
        .unwrap();

        assert_eq!(config.flake_dir, expand_home_path("~/workspace/nix-config"));
        assert_eq!(config.home_flake, "niri-hdmi");
        assert_eq!(config.nixos_flake, "aorus");
        assert_eq!(config.home_flake_dir, None);
        assert_eq!(config.nixos_flake_dir, None);
        assert_eq!(config.home_flake_dir(), config.flake_dir.as_path());
        assert_eq!(config.nixos_flake_dir(), config.flake_dir.as_path());
        assert_eq!(config.dix_binary, None);
        assert_eq!(config.style_css, None);
        assert!(!config.show_demo);
    }

    #[test]
    fn parses_optional_flake_dirs() {
        let config = parse_config(
            "\
flake_dir=$HOME/workspace/nix-config
home_flake_dir=$HOME/workspace/home-config
nixos_flake_dir=~/workspace/nixos-config
home_flake=niri-hdmi
nixos_flake=aorus
",
            Path::new("sunix.toml"),
        )
        .unwrap();

        assert_eq!(
            config.home_flake_dir,
            Some(expand_home_path("$HOME/workspace/home-config"))
        );
        assert_eq!(
            config.nixos_flake_dir,
            Some(expand_home_path("~/workspace/nixos-config"))
        );
        assert_eq!(
            config.home_flake_dir(),
            expand_home_path("$HOME/workspace/home-config").as_path()
        );
        assert_eq!(
            config.nixos_flake_dir(),
            expand_home_path("~/workspace/nixos-config").as_path()
        );
    }

    #[test]
    fn ignores_empty_optional_flake_dirs() {
        let config = parse_config(
            "\
flake_dir=$HOME/workspace/nix-config
home_flake_dir=
nixos_flake_dir=
home_flake=niri-hdmi
nixos_flake=aorus
",
            Path::new("sunix.toml"),
        )
        .unwrap();

        assert_eq!(config.home_flake_dir, None);
        assert_eq!(config.nixos_flake_dir, None);
        assert_eq!(config.home_flake_dir(), config.flake_dir.as_path());
        assert_eq!(config.nixos_flake_dir(), config.flake_dir.as_path());
    }

    #[test]
    fn parses_optional_dix_binary() {
        let config = parse_config(
            "\
flake_dir=$HOME/workspace/nix-config
home_flake=niri-hdmi
nixos_flake=aorus
dix_binary=/nix/store/6ziw66nh8a4b6nwrqmj0n80nsdxz5m61-dix-2.1.0/bin/dix
",
            Path::new("sunix.toml"),
        )
        .unwrap();

        assert_eq!(
            config.dix_binary,
            Some(PathBuf::from(
                "/nix/store/6ziw66nh8a4b6nwrqmj0n80nsdxz5m61-dix-2.1.0/bin/dix"
            ))
        );
        assert!(!config.show_demo);
    }

    #[test]
    fn ignores_empty_optional_dix_binary() {
        let config = parse_config(
            "\
flake_dir=$HOME/workspace/nix-config
home_flake=niri-hdmi
nixos_flake=aorus
dix_binary=
",
            Path::new("sunix.toml"),
        )
        .unwrap();

        assert_eq!(config.dix_binary, None);
        assert!(!config.show_demo);
    }

    #[test]
    fn parses_optional_style_css() {
        let config = parse_config(
            "\
flake_dir=$HOME/workspace/nix-config
home_flake=niri-hdmi
nixos_flake=aorus
style_css=$HOME/.config/sunix/style.css
",
            Path::new("sunix.toml"),
        )
        .unwrap();

        assert_eq!(
            config.style_css,
            Some(expand_home_path("$HOME/.config/sunix/style.css"))
        );
    }

    #[test]
    fn ignores_empty_optional_style_css() {
        let config = parse_config(
            "\
flake_dir=$HOME/workspace/nix-config
home_flake=niri-hdmi
nixos_flake=aorus
style_css=
",
            Path::new("sunix.toml"),
        )
        .unwrap();

        assert_eq!(config.style_css, None);
    }

    #[test]
    fn parses_optional_show_demo() {
        let config = parse_config(
            "\
flake_dir=$HOME/workspace/nix-config
home_flake=niri-hdmi
nixos_flake=aorus
show_demo=true
",
            Path::new("sunix.toml"),
        )
        .unwrap();

        assert!(config.show_demo);
    }

    #[test]
    fn ignores_empty_optional_show_demo() {
        let config = parse_config(
            "\
flake_dir=$HOME/workspace/nix-config
home_flake=niri-hdmi
nixos_flake=aorus
show_demo=
",
            Path::new("sunix.toml"),
        )
        .unwrap();

        assert!(!config.show_demo);
    }

    #[test]
    fn rejects_invalid_optional_show_demo() {
        let err = parse_config(
            "\
flake_dir=$HOME/workspace/nix-config
home_flake=niri-hdmi
nixos_flake=aorus
show_demo=yes
",
            Path::new("sunix.toml"),
        )
        .unwrap_err();

        assert!(err.contains("invalid optional `show_demo` value `yes`"));
    }

    #[test]
    fn rejects_missing_fields() {
        let err = parse_config(
            "\
flake_dir=$HOME/workspace/nix-config
home_flake=niri-hdmi
",
            Path::new("sunix.toml"),
        )
        .unwrap_err();

        assert!(err.contains("missing mandatory `nixos_flake` field"));
    }

    #[test]
    fn rejects_empty_fields() {
        let err = parse_config(
            "\
flake_dir=$HOME/workspace/nix-config
home_flake=
nixos_flake=aorus
",
            Path::new("sunix.toml"),
        )
        .unwrap_err();

        assert!(err.contains("empty mandatory `home_flake` field"));
    }
}
