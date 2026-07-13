use crate::manifest::ComponentManifest;
use anyhow::{Context, Result};
use braid_ports::{ComponentInfo, ComponentRegistry};
use std::collections::HashMap;
use std::path::Path;

/// Loads components from a directory tree where each subdirectory is a component.
///
/// Layout:
/// ```text
/// <root>/
///   hello-world/
///     component.toml
///     commands/
///     prompts/
/// ```
pub struct FileSystemRegistry {
    components: HashMap<String, ComponentManifest>,
}

impl FileSystemRegistry {
    /// Scan `root` — each immediate subdirectory containing `component.toml` is loaded.
    pub fn from_dir(root: &Path) -> Result<Self> {
        let mut components = HashMap::new();
        let entries = std::fs::read_dir(root)
            .with_context(|| format!("reading component directory {}", root.display()))?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if !path.join("component.toml").exists() {
                continue;
            }
            let manifest = ComponentManifest::load(&path)
                .with_context(|| format!("loading component at {}", path.display()))?;
            tracing::debug!(component = %manifest.name, version = %manifest.version, "loaded component");
            components.insert(manifest.name.clone(), manifest);
        }

        Ok(Self { components })
    }
}

impl FileSystemRegistry {
    /// Direct access to a loaded manifest (includes file-reading methods).
    pub fn get_manifest(&self, name: &str) -> Option<&ComponentManifest> {
        self.components.get(name)
    }
}

impl ComponentRegistry for FileSystemRegistry {
    fn load(&mut self, path: &Path) -> Result<()> {
        let manifest = ComponentManifest::load(path)?;
        self.components.insert(manifest.name.clone(), manifest);
        Ok(())
    }

    fn get(&self, name: &str) -> Option<ComponentInfo> {
        self.components.get(name).map(|m| ComponentInfo {
            name: m.name.clone(),
            version: m.version.clone(),
            description: m.description.clone(),
        })
    }

    fn list(&self) -> Vec<ComponentInfo> {
        let mut v: Vec<_> = self.components.values().collect();
        v.sort_by(|a, b| a.name.cmp(&b.name));
        v.into_iter()
            .map(|m| ComponentInfo {
                name: m.name.clone(),
                version: m.version.clone(),
                description: m.description.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn write_component(root: &Path, name: &str) {
        let dir = root.join(name);
        fs::create_dir_all(dir.join("commands")).unwrap();
        fs::write(
            dir.join("component.toml"),
            format!(
                r#"
[component]
name = "{name}"
version = "0.1.0"
"#
            ),
        )
        .unwrap();
    }

    #[test]
    fn from_dir_loads_all_components() {
        let tmp = tempdir().unwrap();
        write_component(tmp.path(), "alpha");
        write_component(tmp.path(), "beta");
        let reg = FileSystemRegistry::from_dir(tmp.path()).unwrap();
        assert_eq!(reg.list().len(), 2);
    }

    #[test]
    fn get_returns_loaded_component() {
        let tmp = tempdir().unwrap();
        write_component(tmp.path(), "alpha");
        let reg = FileSystemRegistry::from_dir(tmp.path()).unwrap();
        assert!(reg.get("alpha").is_some());
        assert!(reg.get("missing").is_none());
    }

    #[test]
    fn list_is_sorted_by_name() {
        let tmp = tempdir().unwrap();
        write_component(tmp.path(), "zebra");
        write_component(tmp.path(), "alpha");
        let reg = FileSystemRegistry::from_dir(tmp.path()).unwrap();
        let names: Vec<_> = reg.list().into_iter().map(|c| c.name).collect();
        assert_eq!(names, ["alpha", "zebra"]);
    }

    #[test]
    fn non_component_subdirs_are_skipped() {
        let tmp = tempdir().unwrap();
        fs::create_dir(tmp.path().join("not-a-component")).unwrap();
        write_component(tmp.path(), "real");
        let reg = FileSystemRegistry::from_dir(tmp.path()).unwrap();
        assert_eq!(reg.list().len(), 1);
    }

    #[test]
    fn load_adds_component_by_path() {
        let tmp = tempdir().unwrap();
        write_component(tmp.path(), "dynamic");
        let mut reg = FileSystemRegistry::from_dir(tmp.path()).unwrap();
        let extra_root = tempdir().unwrap();
        write_component(extra_root.path(), "late");
        let late_path = extra_root.path().join("late");
        reg.load(&late_path).unwrap();
        assert!(reg.get("late").is_some());
    }
}
