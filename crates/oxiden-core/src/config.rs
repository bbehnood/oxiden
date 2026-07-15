//! User-configurable editor settings, loaded from an optional config file.
//!
//! The file lives at the OS's standard config directory — e.g.
//! `~/.config/oxiden/config.toml` on Linux, following the XDG Base
//! Directory spec (see [`Config::default_path`]) — and uses a flat,
//! `key = value` syntax, one setting per line, with `#` comments. It's a
//! subset of TOML (no tables, no nesting) rather than a full TOML parser,
//! in keeping with this project's preference for hand-rolled parsing over
//! pulling in a crate for a handful of settings (see the `--backend` flag
//! parsing in the `oxiden` binary). A config file is entirely optional:
//! every key defaults to something reasonable (see [`Default`]), and a
//! missing file is treated exactly like an empty one.
//!
//! Example file:
//!
//! ```toml
//! tab_width = 2
//! insert_spaces_for_tab = true
//! backend = "ropey"
//! ```

use std::fmt;
use std::path::{Path, PathBuf};

/// All user-configurable editor settings. Every field has a sensible
/// default (see [`Default`]), so a config file only needs to mention the
/// keys it wants to override.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// How many columns a tab advances to when displayed, and how many
    /// spaces it's replaced with when [`Self::insert_spaces_for_tab`] is
    /// set. Key: `tab_width`. Default: `4`.
    pub tab_width: usize,

    /// Whether pressing Tab inserts `tab_width` spaces instead of a
    /// literal tab character. Key: `insert_spaces_for_tab`.
    /// Default: `false`.
    pub insert_spaces_for_tab: bool,

    /// Which [`Backend`] to start with when `--backend` isn't given on
    /// the command line. Key: `backend`. Default: [`Backend::Ropey`].
    pub backend: Backend,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tab_width: 4,
            insert_spaces_for_tab: false,
            backend: Backend::Ropey,
        }
    }
}

/// Which [`oxiden_buffer::TextStorage`] implementation to run the editor
/// with, selectable via config or the `--backend` CLI flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// One `String` per line — the simplest and best-tested backend.
    Vec,
    /// A hand-rolled rope.
    Rope,
    /// A rope backed by the `ropey` crate.
    Ropey,
}

impl Backend {
    /// Parses a backend name as accepted in config files and on the
    /// command line (`"vec"`, `"rope"`, or `"ropey"`).
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "vec" => Some(Backend::Vec),
            "rope" => Some(Backend::Rope),
            "ropey" => Some(Backend::Ropey),
            _ => None,
        }
    }
}

impl fmt::Display for Backend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Backend::Vec => "vec",
            Backend::Rope => "rope",
            Backend::Ropey => "ropey",
        })
    }
}

/// What went wrong loading or parsing a config file.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// Reading the config file failed for a reason other than it not
    /// existing (e.g. a permissions error) — a missing file is not an
    /// error, it just means [`Config::default`].
    #[error("couldn't read {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },

    /// The file exists but couldn't be parsed: an unknown key, a
    /// malformed line, or a value of the wrong type.
    #[error("{path}:{line}: {message}")]
    Parse { path: String, line: usize, message: String },
}

impl Config {
    /// The default config file path: `$XDG_CONFIG_HOME/oxiden/config.toml`
    /// if set, otherwise `$HOME/.config/oxiden/config.toml`. An explicit
    /// `$OXIDEN_CONFIG` overrides both, naming the file directly.
    ///
    /// Returns `None` if none of those environment variables are set
    /// (e.g. a minimal container environment) — callers should fall back
    /// to [`Config::default`] in that case, the same as a missing file.
    pub fn default_path() -> Option<PathBuf> {
        if let Some(path) = std::env::var_os("OXIDEN_CONFIG") {
            return Some(PathBuf::from(path));
        }

        let config_dir = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(|home| PathBuf::from(home).join(".config"))
            })?;

        Some(config_dir.join("oxiden").join("config.toml"))
    }

    /// Loads config from [`Self::default_path`], falling back to
    /// [`Config::default`] if there's no path available or no file at
    /// that path. A malformed file is still reported as an error, so a
    /// typo doesn't silently fall back to defaults.
    pub fn load() -> Result<Self, ConfigError> {
        match Self::default_path() {
            Some(path) => Self::load_from(&path),
            None => Ok(Self::default()),
        }
    }

    /// Loads and parses the config file at `path`. A missing file is not
    /// an error — it just means [`Config::default`].
    pub fn load_from(path: &Path) -> Result<Self, ConfigError> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,

            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }

            Err(source) => {
                return Err(ConfigError::Io {
                    path: path.display().to_string(),
                    source,
                });
            }
        };

        Self::parse(&text, path)
    }

    /// Parses `text` as a config file's contents. `path` is only used to
    /// attribute parse errors to a file name — pass whatever's
    /// appropriate (e.g. a placeholder) when parsing text that didn't
    /// come from disk.
    pub fn parse(text: &str, path: &Path) -> Result<Self, ConfigError> {
        let mut config = Self::default();

        for (index, raw_line) in text.lines().enumerate() {
            let line_number = index + 1;
            let line = raw_line.trim();

            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let Some((key, value)) = line.split_once('=') else {
                return Err(ConfigError::Parse {
                    path: path.display().to_string(),
                    line: line_number,
                    message: format!(
                        "expected `key = value`, got {raw_line:?}"
                    ),
                });
            };

            let key = key.trim();
            let value = unquote(value.trim());
            let parse_error = |message: String| ConfigError::Parse {
                path: path.display().to_string(),
                line: line_number,
                message,
            };

            match key {
                "tab_width" => {
                    let width: usize = value.parse().map_err(|_| {
                        parse_error(format!(
                            "tab_width must be a positive integer, \
                             got {value:?}"
                        ))
                    })?;

                    if width == 0 {
                        return Err(parse_error(
                            "tab_width must be at least 1".to_string(),
                        ));
                    }

                    config.tab_width = width;
                }

                "insert_spaces_for_tab" => {
                    config.insert_spaces_for_tab =
                        value.parse().map_err(|_| {
                            parse_error(format!(
                                "insert_spaces_for_tab must be true or \
                                 false, got {value:?}"
                            ))
                        })?;
                }

                "backend" => {
                    config.backend =
                        Backend::parse(value).ok_or_else(|| {
                            parse_error(format!(
                                "unknown backend {value:?} (expected vec, \
                             rope, or ropey)"
                            ))
                        })?;
                }

                other => {
                    return Err(parse_error(format!(
                        "unknown config key {other:?}"
                    )));
                }
            }
        }

        Ok(config)
    }
}

/// Strips one layer of matching double quotes from `value`, if present,
/// so `backend = "ropey"` and `backend = ropey` both parse the same way.
fn unquote(value: &str) -> &str {
    value.strip_prefix('"').and_then(|v| v.strip_suffix('"')).unwrap_or(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<Config, ConfigError> {
        Config::parse(text, Path::new("config.toml"))
    }

    #[test]
    fn empty_text_yields_defaults() {
        assert_eq!(parse("").unwrap(), Config::default());
    }

    #[test]
    fn comments_and_blank_lines_are_ignored() {
        let text = "\n# a comment\n\ntab_width = 2\n";

        assert_eq!(parse(text).unwrap().tab_width, 2);
    }

    #[test]
    fn parses_all_keys() {
        let text = "tab_width = 2\n\
                     insert_spaces_for_tab = true\n\
                     backend = \"rope\"\n";

        let config = parse(text).unwrap();

        assert_eq!(config.tab_width, 2);
        assert!(config.insert_spaces_for_tab);
        assert_eq!(config.backend, Backend::Rope);
    }

    #[test]
    fn unquoted_backend_value_is_accepted() {
        let config = parse("backend = ropey").unwrap();

        assert_eq!(config.backend, Backend::Ropey);
    }

    #[test]
    fn unmentioned_keys_keep_their_default() {
        let config = parse("tab_width = 8").unwrap();

        assert!(!config.insert_spaces_for_tab);
        assert_eq!(config.backend, Backend::Ropey);
    }

    #[test]
    fn unknown_key_is_a_parse_error() {
        let err = parse("nonexistent = 1").unwrap_err();

        assert!(matches!(err, ConfigError::Parse { line: 1, .. }));
    }

    #[test]
    fn malformed_line_is_a_parse_error() {
        let err = parse("just some text").unwrap_err();

        assert!(matches!(err, ConfigError::Parse { line: 1, .. }));
    }

    #[test]
    fn invalid_tab_width_is_a_parse_error() {
        let err = parse("tab_width = wide").unwrap_err();

        assert!(matches!(err, ConfigError::Parse { .. }));
    }

    #[test]
    fn zero_tab_width_is_a_parse_error() {
        let err = parse("tab_width = 0").unwrap_err();

        assert!(matches!(err, ConfigError::Parse { .. }));
    }

    #[test]
    fn invalid_bool_is_a_parse_error() {
        let err = parse("insert_spaces_for_tab = yes").unwrap_err();

        assert!(matches!(err, ConfigError::Parse { .. }));
    }

    #[test]
    fn invalid_backend_is_a_parse_error() {
        let err = parse("backend = quantum").unwrap_err();

        assert!(matches!(err, ConfigError::Parse { .. }));
    }

    #[test]
    fn error_line_number_matches_the_offending_line() {
        let text = "tab_width = 2\nbackend = nonsense\n";

        let err = parse(text).unwrap_err();

        assert!(matches!(err, ConfigError::Parse { line: 2, .. }));
    }

    #[test]
    fn missing_file_yields_defaults() {
        let config =
            Config::load_from(Path::new("/nonexistent/oxiden/config.toml"))
                .unwrap();

        assert_eq!(config, Config::default());
    }

    #[test]
    fn backend_display_round_trips_through_parse() {
        for backend in [Backend::Vec, Backend::Rope, Backend::Ropey] {
            assert_eq!(Backend::parse(&backend.to_string()), Some(backend));
        }
    }
}
