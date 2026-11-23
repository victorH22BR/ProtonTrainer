//! Steam installation, library and installed game discovery.

use pt_core::AppId;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamGame {
    pub app_id: AppId,
    pub name: String,
    pub install_dir: PathBuf,
    pub library_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamInstallation {
    pub root: PathBuf,
    pub libraries: Vec<PathBuf>,
}

#[derive(Debug, thiserror::Error)]
pub enum SteamError {
    #[error("could not read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid Steam VDF in {path}: {message}")]
    InvalidVdf { path: PathBuf, message: String },
}

const MAX_VDF_BYTES: u64 = 8 * 1024 * 1024;
const MAX_VDF_TOKENS: usize = 200_000;
const MAX_VDF_DEPTH: usize = 64;

/// Find validated Steam roots, including native, XDG and Flatpak installations.
pub fn discover_steam_roots(configured: Option<&Path>) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = configured {
        candidates.push(path.to_path_buf());
    }
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
        candidates.push(PathBuf::from(xdg).join("Steam"));
    }
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        candidates.extend([
            home.join(".steam/steam"),
            home.join(".steam/root"),
            home.join(".local/share/Steam"),
            home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
        ]);
    }
    validated_roots(candidates)
}

fn validated_roots(candidates: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    let mut seen = BTreeSet::new();
    candidates
        .into_iter()
        .filter_map(|path| {
            let canonical = fs::canonicalize(path).ok()?;
            (canonical.join("steamapps/libraryfolders.vdf").is_file()
                && seen.insert(canonical.clone()))
            .then_some(canonical)
        })
        .collect()
}

pub fn discover_installations(
    configured: Option<&Path>,
) -> Result<Vec<SteamInstallation>, SteamError> {
    discover_steam_roots(configured)
        .into_iter()
        .map(|root| {
            let vdf_path = root.join("steamapps/libraryfolders.vdf");
            let data = read_vdf(&vdf_path)?;
            let mut libraries = vec![root.clone()];
            if let Some(entries) = data.get("libraryfolders").and_then(Node::object) {
                for (key, value) in entries {
                    if key == "0" {
                        continue;
                    }
                    let path = value
                        .get("path")
                        .and_then(Node::string)
                        .or_else(|| value.string());
                    if let Some(path) = path {
                        let candidate = PathBuf::from(path);
                        if candidate.is_absolute() && candidate.join("steamapps").is_dir() {
                            libraries.push(fs::canonicalize(&candidate).unwrap_or(candidate));
                        }
                    }
                }
            }
            libraries.sort();
            libraries.dedup();
            Ok(SteamInstallation { root, libraries })
        })
        .collect()
}

pub fn list_games(installations: &[SteamInstallation]) -> Result<Vec<SteamGame>, SteamError> {
    let mut games = Vec::new();
    for installation in installations {
        for library in &installation.libraries {
            let steamapps = library.join("steamapps");
            let entries = match fs::read_dir(&steamapps) {
                Ok(entries) => entries,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(source) => {
                    return Err(SteamError::Io {
                        path: steamapps,
                        source,
                    })
                }
            };
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                let Some(raw_id) = name
                    .strip_prefix("appmanifest_")
                    .and_then(|s| s.strip_suffix(".acf"))
                else {
                    continue;
                };
                let Ok(app_id) = raw_id.parse::<AppId>() else {
                    continue;
                };
                let path = entry.path();
                let data = match read_vdf(&path) {
                    Ok(data) => data,
                    Err(_) => continue,
                };
                let Some(app) = data.get("AppState") else {
                    continue;
                };
                let Some(name) = app.get("name").and_then(Node::string) else {
                    continue;
                };
                let install_dir = app.get("installdir").and_then(Node::string).unwrap_or(name);
                if !is_single_path_component(install_dir) {
                    continue;
                }
                games.push(SteamGame {
                    app_id,
                    name: name.to_owned(),
                    install_dir: steamapps.join("common").join(install_dir),
                    library_path: library.clone(),
                });
            }
        }
    }
    games.sort_by_key(|a| a.name.to_lowercase());
    games.dedup_by_key(|g| g.app_id);
    Ok(games)
}

fn is_single_path_component(value: &str) -> bool {
    !value.is_empty()
        && !value.contains(['/', '\\'])
        && Path::new(value)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
        && Path::new(value).components().count() == 1
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Node {
    String(String),
    Object(BTreeMap<String, Node>),
}
impl Node {
    fn object(&self) -> Option<&BTreeMap<String, Node>> {
        if let Self::Object(x) = self {
            Some(x)
        } else {
            None
        }
    }
    fn string(&self) -> Option<&str> {
        if let Self::String(x) = self {
            Some(x)
        } else {
            None
        }
    }
    fn get(&self, key: &str) -> Option<&Node> {
        self.object()?.get(key)
    }
}

fn read_vdf(path: &Path) -> Result<Node, SteamError> {
    let file = fs::File::open(path).map_err(|source| SteamError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let size = file
        .metadata()
        .map_err(|source| SteamError::Io {
            path: path.to_path_buf(),
            source,
        })?
        .len();
    if size > MAX_VDF_BYTES {
        return Err(SteamError::InvalidVdf {
            path: path.to_path_buf(),
            message: "file exceeds the 8 MiB parser limit".into(),
        });
    }
    let mut input = String::new();
    file.take(MAX_VDF_BYTES + 1)
        .read_to_string(&mut input)
        .map_err(|source| SteamError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    if input.len() as u64 > MAX_VDF_BYTES {
        return Err(SteamError::InvalidVdf {
            path: path.to_path_buf(),
            message: "file exceeds the 8 MiB parser limit".into(),
        });
    }
    parse_vdf(&input).map_err(|message| SteamError::InvalidVdf {
        path: path.to_path_buf(),
        message,
    })
}

fn parse_vdf(input: &str) -> Result<Node, String> {
    if input.len() as u64 > MAX_VDF_BYTES {
        return Err("input exceeds the 8 MiB parser limit".into());
    }
    let tokens = tokenize(input)?;
    let mut pos = 0;
    let node = parse_object(&tokens, &mut pos, false, 0)?;
    if pos != tokens.len() {
        return Err("unexpected trailing token".into());
    }
    Ok(node)
}

fn parse_object(
    tokens: &[String],
    pos: &mut usize,
    nested: bool,
    depth: usize,
) -> Result<Node, String> {
    if depth > MAX_VDF_DEPTH {
        return Err("VDF nesting exceeds the parser limit".into());
    }
    let mut map = BTreeMap::new();
    while *pos < tokens.len() {
        if tokens[*pos] == "}" {
            if !nested {
                return Err("unexpected closing brace".into());
            }
            *pos += 1;
            return Ok(Node::Object(map));
        }
        let key = tokens[*pos].clone();
        *pos += 1;
        let value = tokens.get(*pos).ok_or("missing value")?;
        *pos += 1;
        let value = if value == "{" {
            parse_object(tokens, pos, true, depth + 1)?
        } else if value == "}" {
            return Err("missing value before closing brace".into());
        } else {
            Node::String(value.clone())
        };
        map.insert(key, value);
    }
    if nested {
        return Err("unclosed object".into());
    }
    Ok(Node::Object(map))
}

fn tokenize(input: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch.is_whitespace() {
            continue;
        }
        if ch == '/' && chars.peek() == Some(&'/') {
            chars.next();
            while chars.next().is_some_and(|c| c != '\n') {}
            continue;
        }
        if ch == '{' || ch == '}' {
            tokens.push(ch.to_string());
            if tokens.len() > MAX_VDF_TOKENS {
                return Err("VDF token count exceeds the parser limit".into());
            }
            continue;
        }
        if ch == '"' {
            let mut value = String::new();
            let mut closed = false;
            while let Some(c) = chars.next() {
                match c {
                    '"' => {
                        closed = true;
                        break;
                    }
                    '\\' => match chars.next() {
                        Some('"') => value.push('"'),
                        Some('\\') => value.push('\\'),
                        Some(c) => {
                            value.push('\\');
                            value.push(c)
                        }
                        None => return Err("unfinished escape".into()),
                    },
                    c => value.push(c),
                }
            }
            if !closed {
                return Err("unterminated quoted string".into());
            }
            tokens.push(value);
            if tokens.len() > MAX_VDF_TOKENS {
                return Err("VDF token count exceeds the parser limit".into());
            }
            continue;
        }
        let mut value = String::from(ch);
        while let Some(&c) = chars.peek() {
            if c.is_whitespace() || c == '{' || c == '}' {
                break;
            }
            value.push(c);
            chars.next();
        }
        tokens.push(value);
        if tokens.len() > MAX_VDF_TOKENS {
            return Err("VDF token count exceeds the parser limit".into());
        }
    }
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_quoted_escaped_nested_vdf() {
        let n = parse_vdf("\"root\" { \"name\" \"Game \\\"One\\\"\" \"nested\" { \"x\" \"1\" } }")
            .unwrap();
        assert_eq!(
            n.get("root").unwrap().get("name").unwrap().string(),
            Some("Game \"One\"")
        );
        assert_eq!(
            n.get("root")
                .unwrap()
                .get("nested")
                .unwrap()
                .get("x")
                .unwrap()
                .string(),
            Some("1")
        );
    }

    #[test]
    fn finds_multiple_roots_and_deduplicates_symlinks() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let first = temp.path().join("Steam A");
        let second = temp.path().join("Steam B");
        fs::create_dir_all(first.join("steamapps")).unwrap();
        fs::create_dir_all(second.join("steamapps")).unwrap();
        fs::write(first.join("steamapps/libraryfolders.vdf"), "{}").unwrap();
        fs::write(second.join("steamapps/libraryfolders.vdf"), "{}").unwrap();
        let alias = temp.path().join("steam-alias");
        symlink(&first, &alias).unwrap();
        let roots = validated_roots([
            first.clone(),
            second.clone(),
            alias,
            temp.path().join("absent"),
        ]);
        assert_eq!(roots.len(), 2);
        assert!(roots.contains(&fs::canonicalize(first).unwrap()));
        assert!(roots.contains(&fs::canonicalize(second).unwrap()));
    }
    #[test]
    fn discovers_games_and_modern_library_entries_from_fixtures() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Steam");
        let other = tmp.path().join("Game Library");
        fs::create_dir_all(root.join("steamapps")).unwrap();
        fs::create_dir_all(other.join("steamapps/common/Other Game")).unwrap();
        fs::write(
            root.join("steamapps/libraryfolders.vdf"),
            format!("\"libraryfolders\" {{ \"0\" {{ \"path\" \"root\" }} \"1\" {{ \"path\" \"{}\" }} }}", other.display()),
        )
        .unwrap();
        fs::write(root.join("steamapps/appmanifest_42.acf"), "\"AppState\" { \"appid\" \"42\" \"name\" \"Example\" \"installdir\" \"Example Game\" }").unwrap();
        fs::write(
            other.join("steamapps/appmanifest_43.acf"),
            "\"AppState\" { \"appid\" \"43\" \"name\" \"Other\" \"installdir\" \"Other Game\" }",
        )
        .unwrap();
        let canonical_root = fs::canonicalize(&root).unwrap();
        let found = discover_installations(Some(&root))
            .unwrap()
            .into_iter()
            .filter(|i| i.root == canonical_root)
            .collect::<Vec<_>>();
        let games = list_games(&found).unwrap();
        assert_eq!(games.len(), 2);
        assert_eq!(games[0].name, "Example");
        assert_eq!(games[0].app_id.get(), 42);
    }

    #[test]
    fn skips_corrupt_manifests_and_rejects_traversal_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Steam");
        let steamapps = root.join("steamapps");
        fs::create_dir_all(&steamapps).unwrap();
        fs::write(
            steamapps.join("libraryfolders.vdf"),
            "\"libraryfolders\" { \"0\" { \"path\" \"root\" } }",
        )
        .unwrap();
        fs::write(
            steamapps.join("appmanifest_51.acf"),
            "\"AppState\" { \"appid\" \"51\" \"name\" \"Broken\" ",
        )
        .unwrap();
        fs::write(steamapps.join("appmanifest_52.acf"), "\"AppState\" { \"appid\" \"52\" \"name\" \"Escape\" \"installdir\" \"../../outside\" }").unwrap();
        let installation = SteamInstallation {
            root: root.clone(),
            libraries: vec![root],
        };
        assert!(list_games(&[installation]).unwrap().is_empty());
    }

    #[test]
    fn does_not_accept_relative_or_traversing_library_paths() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("Steam");
        fs::create_dir_all(root.join("steamapps")).unwrap();
        let outside = temp.path().join("outside");
        fs::create_dir_all(outside.join("steamapps")).unwrap();
        fs::write(
            root.join("steamapps/libraryfolders.vdf"),
            "\"libraryfolders\" { \"0\" { \"path\" \"relative-lib\" } \"1\" { \"path\" \"../outside\" } }",
        ).unwrap();
        let installations = discover_installations(Some(&root)).unwrap();
        let installation = installations
            .into_iter()
            .find(|i| i.root == fs::canonicalize(&root).unwrap())
            .unwrap();
        assert_eq!(
            installation.libraries,
            vec![fs::canonicalize(root).unwrap()]
        );
    }
}
