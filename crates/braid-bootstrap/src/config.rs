use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BraidConfig {
    pub provider: ProviderConfig,
    pub session: SessionConfig,
    pub context: ContextConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub default: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub max_turns: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextConfig {
    pub budget_tokens: usize,
}

impl Default for BraidConfig {
    fn default() -> Self {
        Self {
            provider: ProviderConfig {
                default: "openai".into(),
                model: "gpt-4o".into(),
            },
            session: SessionConfig { max_turns: 20 },
            context: ContextConfig {
                budget_tokens: 2000,
            },
        }
    }
}

impl BraidConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_expected_values() {
        let cfg = BraidConfig::default();
        assert_eq!(cfg.provider.default, "openai");
        assert_eq!(cfg.provider.model, "gpt-4o");
        assert_eq!(cfg.session.max_turns, 20);
        assert_eq!(cfg.context.budget_tokens, 2000);
    }

    #[test]
    fn config_round_trips_through_toml() {
        let original = BraidConfig::default();
        let serialized = toml::to_string_pretty(&original).unwrap();
        let loaded: BraidConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(loaded.provider.default, original.provider.default);
        assert_eq!(loaded.provider.model, original.provider.model);
        assert_eq!(loaded.session.max_turns, original.session.max_turns);
        assert_eq!(loaded.context.budget_tokens, original.context.budget_tokens);
    }

    #[test]
    fn load_reads_toml_from_disk() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        let cfg = BraidConfig::default();
        std::fs::write(&path, toml::to_string_pretty(&cfg).unwrap()).unwrap();
        let loaded = BraidConfig::load(&path).unwrap();
        assert_eq!(loaded.provider.model, "gpt-4o");
    }
}
