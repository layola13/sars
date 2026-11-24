use std::path::{Path, PathBuf};

pub trait ImportResolver {
    fn resolve(&self, path: &str, from: &str) -> Option<String>;
}

pub struct FsResolver {
    pub roots: Vec<PathBuf>,
}

impl FsResolver {
    pub fn new(roots: Vec<PathBuf>) -> Self {
        Self { roots }
    }

    fn read_file(path: &Path) -> Option<String> {
        if !path.is_file() {
            return None;
        }
        std::fs::read_to_string(path).ok()
    }
}

impl ImportResolver for FsResolver {
    fn resolve(&self, path: &str, from: &str) -> Option<String> {
        let local = match Path::new(from).parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent.join(path),
            _ => PathBuf::from(path),
        };
        if let Some(text) = Self::read_file(&local) {
            return Some(text);
        }
        for root in &self.roots {
            let candidate = root.join(path);
            if let Some(text) = Self::read_file(&candidate) {
                return Some(text);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct Dir(PathBuf);

    impl Dir {
        fn new() -> Self {
            let n = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir()
                .join(format!("sa_flatten_resolver_{}_{}", std::process::id(), n));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn resolves_relative_to_from_first() {
        let dir = Dir::new();
        let local = dir.0.join("local.sa");
        let root = dir.0.join("root");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("shared.sa"), "root version").unwrap();
        std::fs::write(&local, "local version").unwrap();
        let resolver = FsResolver::new(vec![root]);
        let main = dir.0.join("main.sa");
        assert_eq!(
            resolver
                .resolve("local.sa", main.to_str().unwrap())
                .as_deref(),
            Some("local version")
        );
    }

    #[test]
    fn falls_back_to_roots() {
        let dir = Dir::new();
        let root = dir.0.join("root");
        std::fs::create_dir_all(root.join("lib")).unwrap();
        std::fs::write(root.join("lib/util.sa"), "root version").unwrap();
        let resolver = FsResolver::new(vec![root]);
        assert_eq!(
            resolver.resolve("lib/util.sa", "main.sa").as_deref(),
            Some("root version")
        );
    }

    #[test]
    fn returns_none_for_missing_file() {
        let resolver = FsResolver::new(vec![]);
        assert!(resolver.resolve("missing.sa", "main.sa").is_none());
    }
}
