use anyhow::Result;
use std::path::Path;

/// Creates the braid directory and config.toml if they don't exist.
/// `braid_dir` is typically `$HOME/.braid`.
pub fn run(braid_dir: &Path) -> Result<()> {
    if !braid_dir.exists() {
        std::fs::create_dir_all(braid_dir)?;
        println!("created  {}", braid_dir.display());
    }

    let config_path = braid_dir.join("config.toml");
    if config_path.exists() {
        println!("skipped  {} (already exists)", config_path.display());
    } else {
        let content = toml::to_string_pretty(&crate::config::BraidConfig::default())?;
        std::fs::write(&config_path, content)?;
        println!("created  {}", config_path.display());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_creates_dir_and_config() {
        let tmp = tempfile::tempdir().unwrap();
        let braid_dir = tmp.path().join(".braid");
        assert!(!braid_dir.exists());

        run(&braid_dir).unwrap();

        assert!(braid_dir.exists());
        let config_path = braid_dir.join("config.toml");
        assert!(config_path.exists());
        let content = std::fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("gpt-4o"));
    }

    #[test]
    fn setup_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let braid_dir = tmp.path().join(".braid");

        run(&braid_dir).unwrap();

        let config_path = braid_dir.join("config.toml");
        std::fs::write(&config_path, "# sentinel\n").unwrap();

        run(&braid_dir).unwrap();

        let content = std::fs::read_to_string(&config_path).unwrap();
        assert_eq!(content, "# sentinel\n", "second run must not overwrite existing config");
    }
}
