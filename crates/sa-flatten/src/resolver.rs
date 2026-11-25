use std::path::{Path, PathBuf};

pub trait ImportResolver {
    fn resolve(&self, path: &str, from: &str) -> Option<String>;
    fn resolve_path(&self, path: &str, from: &str) -> Option<String> {
        let _ = (path, from);
        None
    }

    /// Resolve an import the way the flattener does: relative to the
    /// importing file's directory, shedding one leading path component at
    /// a time, plus package roots when the path is rooted at one. `allow_roots`
    /// is true only for imports in the top-level source file.
    fn resolve_child(
        &self,
        path: &str,
        from: &str,
        allow_roots: bool,
    ) -> Option<(String, String)> {
        let content = self.resolve(path, from)?;
        let resolved = self
            .resolve_path(path, from)
            .unwrap_or_else(|| path.to_string());
        let _ = allow_roots;
        Some((content, resolved))
    }
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

    fn find_path(&self, path: &str, from: &str) -> Option<PathBuf> {
        let local = match Path::new(from).parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent.join(path),
            _ => PathBuf::from(path),
        };
        if local.is_file() {
            return Some(local);
        }
        for root in &self.roots {
            let candidate = root.join(path);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        None
    }

    fn find_local_path(&self, path: &str, from: &str) -> Option<PathBuf> {
        let base = match Path::new(from).parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
            _ => PathBuf::new(),
        };
        let mut current = path.to_string();
        loop {
            let candidate = base.join(&current);
            if candidate.is_file() {
                return Some(candidate);
            }
            match current.find('/') {
                Some(index) => current = current[index + 1..].to_string(),
                None => return None,
            }
        }
    }

    fn is_package_path(&self, path: &str) -> bool {
        let first = path.split('/').next().unwrap_or("");
        self.roots
            .iter()
            .any(|root| root.file_name().and_then(|name| name.to_str()) == Some(first))
    }

    fn read_path(path: &Path) -> Option<(String, String)> {
        let content = Self::read_file(path)?;
        Some((content, path.to_string_lossy().to_string()))
    }
}

impl ImportResolver for FsResolver {
    fn resolve(&self, path: &str, from: &str) -> Option<String> {
        self.find_path(path, from).and_then(|p| Self::read_file(&p))
    }

    fn resolve_path(&self, path: &str, from: &str) -> Option<String> {
        self.find_path(path, from)
            .map(|p| p.to_string_lossy().to_string())
    }

    fn resolve_child(
        &self,
        path: &str,
        from: &str,
        allow_roots: bool,
    ) -> Option<(String, String)> {
        if let Some(local) = self.find_local_path(path, from) {
            return Self::read_path(&local);
        }
        if allow_roots || self.is_package_path(path) {
            for root in &self.roots {
                let candidate = root.join(path);
                if let Some(resolved) = Self::read_path(&candidate) {
                    return Some(resolved);
                }
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

    #[test]
    fn child_import_sheds_leading_path_components() {
        let dir = Dir::new();
        let east = dir.0.join("east");
        let west = dir.0.join("west");
        std::fs::create_dir_all(&east).unwrap();
        std::fs::create_dir_all(&west).unwrap();
        std::fs::write(west.join("beta.sa"), "west version").unwrap();
        let resolver = FsResolver::new(vec![dir.0.clone()]);
        let from = east.join("alpha.sa");
        assert_eq!(
            resolver
                .resolve_child("east/../west/beta.sa", from.to_str().unwrap(), false)
                .map(|(source, _)| source),
            Some("west version".to_string())
        );
    }

    #[test]
    fn child_import_rejects_bare_crossdir_path() {
        let dir = Dir::new();
        let east = dir.0.join("east");
        let west = dir.0.join("west");
        std::fs::create_dir_all(&east).unwrap();
        std::fs::create_dir_all(&west).unwrap();
        std::fs::write(west.join("beta.sa"), "west version").unwrap();
        let resolver = FsResolver::new(vec![dir.0.clone()]);
        let from = east.join("alpha.sa");
        assert!(resolver
            .resolve_child("west/beta.sa", from.to_str().unwrap(), false)
            .is_none());
        assert!(resolver
            .resolve_child("west/beta.sa", from.to_str().unwrap(), true)
            .is_some());
    }
}
