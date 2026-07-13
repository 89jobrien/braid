use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestMeta {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandEntry {
    pub name: String,
    pub file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptEntry {
    pub name: String,
    pub file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawManifest {
    pub component: ManifestMeta,
    #[serde(default)]
    pub commands: Vec<CommandEntry>,
    #[serde(default)]
    pub prompts: Vec<PromptEntry>,
}

/// A loaded component: manifest metadata plus resolved file contents.
#[derive(Debug, Clone)]
pub struct ComponentManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub commands: Vec<CommandEntry>,
    pub prompts: Vec<PromptEntry>,
    /// Absolute path to the component directory.
    pub root: PathBuf,
}

impl ComponentManifest {
    pub fn load(dir: &Path) -> Result<Self> {
        let manifest_path = dir.join("component.toml");
        let content = std::fs::read_to_string(&manifest_path)
            .with_context(|| format!("reading {}", manifest_path.display()))?;
        let raw: RawManifest = toml::from_str(&content)
            .with_context(|| format!("parsing {}", manifest_path.display()))?;

        // Validate that all referenced files exist.
        for cmd in &raw.commands {
            let p = dir.join(&cmd.file);
            anyhow::ensure!(p.exists(), "command file not found: {}", p.display());
        }
        for prompt in &raw.prompts {
            let p = dir.join(&prompt.file);
            anyhow::ensure!(p.exists(), "prompt file not found: {}", p.display());
        }

        Ok(Self {
            name: raw.component.name,
            version: raw.component.version,
            description: raw.component.description,
            commands: raw.commands,
            prompts: raw.prompts,
            root: dir.to_path_buf(),
        })
    }

    /// Read a command's content by name.
    pub fn read_command(&self, name: &str) -> Result<String> {
        let entry = self
            .commands
            .iter()
            .find(|c| c.name == name)
            .with_context(|| format!("no command '{name}' in component '{}'", self.name))?;
        let path = self.root.join(&entry.file);
        std::fs::read_to_string(&path)
            .with_context(|| format!("reading command file {}", path.display()))
    }

    /// Read a prompt's content by name.
    pub fn read_prompt(&self, name: &str) -> Result<String> {
        let entry = self
            .prompts
            .iter()
            .find(|p| p.name == name)
            .with_context(|| format!("no prompt '{name}' in component '{}'", self.name))?;
        let path = self.root.join(&entry.file);
        std::fs::read_to_string(&path)
            .with_context(|| format!("reading prompt file {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn write_fixture(dir: &Path) {
        fs::create_dir_all(dir.join("commands")).unwrap();
        fs::create_dir_all(dir.join("prompts")).unwrap();
        fs::write(
            dir.join("component.toml"),
            r#"
[component]
name = "test-comp"
version = "0.1.0"
description = "A test component"

[[commands]]
name = "run"
file = "commands/run.md"

[[prompts]]
name = "system"
file = "prompts/system.txt"
"#,
        )
        .unwrap();
        fs::write(dir.join("commands/run.md"), "# run\nDoes stuff.").unwrap();
        fs::write(dir.join("prompts/system.txt"), "You are a helper.").unwrap();
    }

    #[test]
    fn load_parses_manifest_fields() {
        let tmp = tempdir().unwrap();
        write_fixture(tmp.path());
        let m = ComponentManifest::load(tmp.path()).unwrap();
        assert_eq!(m.name, "test-comp");
        assert_eq!(m.version, "0.1.0");
        assert_eq!(m.commands.len(), 1);
        assert_eq!(m.prompts.len(), 1);
    }

    #[test]
    fn read_command_returns_file_content() {
        let tmp = tempdir().unwrap();
        write_fixture(tmp.path());
        let m = ComponentManifest::load(tmp.path()).unwrap();
        let content = m.read_command("run").unwrap();
        assert!(content.contains("run"));
    }

    #[test]
    fn read_prompt_returns_file_content() {
        let tmp = tempdir().unwrap();
        write_fixture(tmp.path());
        let m = ComponentManifest::load(tmp.path()).unwrap();
        let content = m.read_prompt("system").unwrap();
        assert!(content.contains("helper"));
    }

    #[test]
    fn missing_command_file_errors_at_load() {
        let tmp = tempdir().unwrap();
        fs::write(
            tmp.path().join("component.toml"),
            r#"
[component]
name = "broken"
version = "0.1.0"

[[commands]]
name = "oops"
file = "commands/missing.md"
"#,
        )
        .unwrap();
        let err = ComponentManifest::load(tmp.path()).unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn read_unknown_command_errors() {
        let tmp = tempdir().unwrap();
        write_fixture(tmp.path());
        let m = ComponentManifest::load(tmp.path()).unwrap();
        let err = m.read_command("nonexistent").unwrap_err();
        assert!(err.to_string().contains("no command"));
    }
}
