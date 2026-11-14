//! Steam installs and libraries.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Where Steam keeps its root: native, the `~/.steam` links, and Flatpak.
pub fn default_roots() -> Vec<PathBuf> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return Vec::new();
    };
    let mut roots = Vec::new();
    for root in [".local/share/Steam", ".steam/steam", ".steam/root", ".var/app/com.valvesoftware.Steam/.local/share/Steam"] {
        if let Ok(root) = home.join(root).canonicalize()
            && !roots.contains(&root)
        {
            roots.push(root);
        }
    }
    roots
}

/// Install directory of `app_id` in whichever library of `roots` holds it.
pub fn app_install_dir(roots: &[PathBuf], app_id: &str) -> Option<PathBuf> {
    roots.iter().flat_map(|root| libraries(root)).find_map(|library| {
        let manifest = fs::read_to_string(library.join(format!("steamapps/appmanifest_{app_id}.acf"))).ok()?;
        let install_dir = vdf::parse(&manifest).get("AppState")?.get("installdir")?.as_str()?.to_owned();
        Some(library.join("steamapps/common").join(install_dir))
    })
}

/// Library folders listed in `root`'s `libraryfolders.vdf`.
fn libraries(root: &Path) -> Vec<PathBuf> {
    let Ok(text) = fs::read_to_string(root.join("steamapps/libraryfolders.vdf")) else {
        return Vec::new();
    };
    let doc = vdf::parse(&text);
    let Some(folders) = doc.get("libraryfolders").and_then(vdf::Value::as_map) else {
        return Vec::new();
    };
    folders.values().filter_map(|f| f.get("path")?.as_str()).map(PathBuf::from).collect()
}

/// Valve's KeyValues text format: `"key" "value"` and `"key" { … }`.
mod vdf {
    use super::HashMap;

    #[derive(Debug)]
    pub enum Value {
        Str(String),
        Map(HashMap<String, Value>),
    }

    impl Value {
        pub fn get(&self, key: &str) -> Option<&Value> {
            self.as_map()?.get(key)
        }
        pub fn as_map(&self) -> Option<&HashMap<String, Value>> {
            match self {
                Value::Map(map) => Some(map),
                Value::Str(_) => None,
            }
        }
        pub fn as_str(&self) -> Option<&str> {
            match self {
                Value::Str(s) => Some(s),
                Value::Map(_) => None,
            }
        }
    }

    /// Parses leniently: malformed input yields whatever was read before it.
    pub fn parse(text: &str) -> Value {
        let mut tokens = Tokens(text.chars().peekable());
        Value::Map(map(&mut tokens))
    }

    enum Token {
        Str(String),
        Open,
        Close,
    }

    struct Tokens<'a>(std::iter::Peekable<std::str::Chars<'a>>);

    impl Iterator for Tokens<'_> {
        type Item = Token;
        fn next(&mut self) -> Option<Token> {
            loop {
                match self.0.next()? {
                    '{' => return Some(Token::Open),
                    '}' => return Some(Token::Close),
                    '"' => {
                        let mut s = String::new();
                        while let Some(c) = self.0.next() {
                            match c {
                                '"' => break,
                                '\\' => s.extend(self.0.next()),
                                c => s.push(c),
                            }
                        }
                        return Some(Token::Str(s));
                    }
                    _ => {}
                }
            }
        }
    }

    fn map(tokens: &mut Tokens) -> HashMap<String, Value> {
        let mut map = HashMap::new();
        while let Some(Token::Str(key)) = tokens.next() {
            match tokens.next() {
                Some(Token::Str(value)) => {
                    map.insert(key, Value::Str(value));
                }
                Some(Token::Open) => {
                    map.insert(key, Value::Map(self::map(tokens)));
                }
                _ => break,
            }
        }
        map
    }
}
