//! Proton compatibility prefix and selected tool detection.

use pt_core::AppId;
use pt_steam::SteamGame;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtonInfo {
    pub prefix: Option<PathBuf>,
    pub version: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProtonError {
    #[error("could not inspect {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

const MAX_CONFIG_VDF_BYTES: u64 = 8 * 1024 * 1024;
const MAX_CONFIG_VDF_TOKENS: usize = 200_000;

/// Resolve a game prefix from the library containing that game's manifest.
/// A directory is accepted only when it has Wine's `system.reg` marker.
pub fn detect_game_proton(game: &SteamGame, steam_root: &Path) -> Result<ProtonInfo, ProtonError> {
    let compat = game
        .library_path
        .join("steamapps/compatdata")
        .join(game.app_id.to_string());
    let candidate = compat.join("pfx");
    let prefix = if candidate.join("system.reg").is_file() {
        Some(candidate)
    } else {
        None
    };
    let version_path = compat.join("version");
    let version = match fs::read_to_string(&version_path) {
        Ok(v) => Some(v.trim().to_owned()).filter(|v| !v.is_empty()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(source) => {
            return Err(ProtonError::Io {
                path: version_path,
                source,
            })
        }
    };
    let version = match version {
        Some(v) => Some(v),
        None => selected_compat_tool(steam_root, game.app_id)?,
    };
    Ok(ProtonInfo { prefix, version })
}

/// Look up `CompatToolMapping` in Steam's config.vdf for the selected AppID.
pub fn selected_compat_tool(
    steam_root: &Path,
    app_id: AppId,
) -> Result<Option<String>, ProtonError> {
    let path = steam_root.join("config/config.vdf");
    let file = match fs::File::open(&path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(ProtonError::Io { path, source }),
    };
    let size = file
        .metadata()
        .map_err(|source| ProtonError::Io {
            path: path.clone(),
            source,
        })?
        .len();
    if size > MAX_CONFIG_VDF_BYTES {
        return Err(ProtonError::Io {
            path: path.clone(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "config.vdf exceeds the 8 MiB parser limit",
            ),
        });
    }
    let mut content = String::new();
    file.take(MAX_CONFIG_VDF_BYTES + 1)
        .read_to_string(&mut content)
        .map_err(|source| ProtonError::Io {
            path: path.clone(),
            source,
        })?;
    if content.len() as u64 > MAX_CONFIG_VDF_BYTES {
        return Err(ProtonError::Io {
            path: path.clone(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "config.vdf exceeds the 8 MiB parser limit",
            ),
        });
    }
    // CompatToolMapping is a small VDF subtree. Reuse the same strict tokenizer
    // semantics locally to avoid coupling domain crates to a parser prematurely.
    let tokens = tokenize(&content).map_err(|_| ProtonError::Io {
        path: path.clone(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid config.vdf"),
    })?;
    let key = app_id.to_string();
    let mut i = 0;
    while i + 1 < tokens.len() {
        if tokens[i].eq_ignore_ascii_case("CompatToolMapping") && tokens[i + 1] == "{" {
            let end = find_close(&tokens, i + 1).unwrap_or(tokens.len());
            for (offset, token) in tokens[i + 2..end].iter().enumerate() {
                if token == &key && tokens.get(i + 3 + offset).is_some_and(|v| v == "{") {
                    let begin = i + 4 + offset;
                    let object_end = find_close(&tokens, begin - 1).unwrap_or(end);
                    if let Some(pos) = tokens[begin..object_end]
                        .windows(2)
                        .position(|w| w[0] == "name")
                    {
                        return Ok(Some(tokens[begin + pos + 1].clone()));
                    }
                }
            }
            return Ok(None);
        }
        i += 1;
    }
    Ok(None)
}

fn tokenize(s: &str) -> Result<Vec<String>, ()> {
    if s.len() as u64 > MAX_CONFIG_VDF_BYTES {
        return Err(());
    }
    let mut t = Vec::new();
    let mut c = s.chars().peekable();
    while let Some(x) = c.next() {
        if x.is_whitespace() {
            continue;
        }
        if x == '/' && c.peek() == Some(&'/') {
            c.next();
            while c.next().is_some_and(|z| z != '\n') {}
            continue;
        }
        if x == '{' || x == '}' {
            t.push(x.to_string());
            if t.len() > MAX_CONFIG_VDF_TOKENS {
                return Err(());
            }
            continue;
        }
        if x == '"' {
            let mut v = String::new();
            let mut done = false;
            while let Some(z) = c.next() {
                match z {
                    '"' => {
                        done = true;
                        break;
                    }
                    '\\' => v.push(c.next().ok_or(())?),
                    z => v.push(z),
                }
            }
            if !done {
                return Err(());
            }
            t.push(v);
            if t.len() > MAX_CONFIG_VDF_TOKENS {
                return Err(());
            }
            continue;
        }
        let mut v = x.to_string();
        while let Some(&z) = c.peek() {
            if z.is_whitespace() || z == '{' || z == '}' {
                break;
            }
            v.push(z);
            c.next();
        }
        t.push(v);
        if t.len() > MAX_CONFIG_VDF_TOKENS {
            return Err(());
        }
    }
    Ok(t)
}
fn find_close(t: &[String], start: usize) -> Option<usize> {
    let mut depth = 0;
    for (i, v) in t.iter().enumerate().skip(start) {
        if v == "{" {
            depth += 1
        } else if v == "}" {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prefix_requires_marker_and_reads_version_file() {
        let d = tempfile::tempdir().unwrap();
        let lib = d.path().join("lib");
        let root = d.path().join("steam");
        let compat = lib.join("steamapps/compatdata/42");
        fs::create_dir_all(compat.join("pfx")).unwrap();
        fs::write(compat.join("pfx/system.reg"), "wine").unwrap();
        fs::write(compat.join("version"), "Proton 9.0\n").unwrap();
        let game = SteamGame {
            app_id: AppId::new(42).unwrap(),
            name: "G".into(),
            install_dir: lib.join("steamapps/common/G"),
            library_path: lib,
        };
        let p = detect_game_proton(&game, &root).unwrap();
        assert!(p.prefix.is_some());
        assert_eq!(p.version.as_deref(), Some("Proton 9.0"));
    }

    #[test]
    fn missing_or_unmarked_prefix_is_not_invented() {
        let d = tempfile::tempdir().unwrap();
        let lib = d.path().join("library");
        let root = d.path().join("steam");
        let compat = lib.join("steamapps/compatdata/42");
        fs::create_dir_all(compat.join("pfx")).unwrap();
        fs::write(compat.join("version"), "Proton 8.0\n").unwrap();
        let game = SteamGame {
            app_id: AppId::new(42).unwrap(),
            name: "G".into(),
            install_dir: lib.join("steamapps/common/G"),
            library_path: lib,
        };
        let detected = detect_game_proton(&game, &root).unwrap();
        assert_eq!(detected.prefix, None);
        assert_eq!(detected.version.as_deref(), Some("Proton 8.0"));
        fs::remove_dir_all(compat.join("pfx")).unwrap();
        assert_eq!(detect_game_proton(&game, &root).unwrap().prefix, None);
    }
    #[test]
    fn reads_selected_tool_from_config_mapping() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join("config")).unwrap();
        fs::write(d.path().join("config/config.vdf"),"\"InstallConfigStore\" { \"Software\" { \"Valve\" { \"Steam\" { \"CompatToolMapping\" { \"42\" { \"name\" \"GE-Proton\" } } } } } }").unwrap();
        assert_eq!(
            selected_compat_tool(d.path(), AppId::new(42).unwrap())
                .unwrap()
                .as_deref(),
            Some("GE-Proton")
        );
    }

    #[test]
    fn selects_a_different_proton_tool_for_each_app_id() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join("config")).unwrap();
        fs::write(
            d.path().join("config/config.vdf"),
            "\"CompatToolMapping\" { \"42\" { \"name\" \"GE-Proton\" } \"43\" { \"name\" \"Proton Experimental\" } }",
        ).unwrap();
        assert_eq!(
            selected_compat_tool(d.path(), AppId::new(43).unwrap())
                .unwrap()
                .as_deref(),
            Some("Proton Experimental")
        );
    }
}
