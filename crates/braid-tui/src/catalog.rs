use braid_components::FileSystemRegistry;
use braid_ports::ComponentRegistry;

#[derive(Debug, Clone)]
pub struct CatalogEntry {
    pub kind: EntryKind,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryKind {
    Component,
    Command,
    Prompt,
}

impl EntryKind {
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Component => "comp",
            Self::Command => "cmd ",
            Self::Prompt => "tmpl",
        }
    }
}

pub struct Catalog {
    pub entries: Vec<CatalogEntry>,
    pub cursor: usize,
    pub filter: String,
}

impl Catalog {
    pub fn load() -> Self {
        let mut entries = Vec::new();

        let home = std::env::var("HOME").unwrap_or_default();
        let components_dir = std::path::PathBuf::from(&home)
            .join(".braid")
            .join("components");

        if components_dir.exists()
            && let Ok(reg) = FileSystemRegistry::from_dir(&components_dir)
        {
            for c in reg.list() {
                for cmd in reg
                    .get_manifest(&c.name)
                    .map_or(&[][..], |m| m.commands.as_slice())
                {
                    entries.push(CatalogEntry {
                        kind: EntryKind::Command,
                        name: format!("{}/{}", c.name, cmd.name),
                        description: c.description.clone(),
                    });
                }
                for prompt in reg
                    .get_manifest(&c.name)
                    .map_or(&[][..], |m| m.prompts.as_slice())
                {
                    entries.push(CatalogEntry {
                        kind: EntryKind::Prompt,
                        name: format!("{}/{}", c.name, prompt.name),
                        description: c.description.clone(),
                    });
                }
                if entries
                    .iter()
                    .all(|e| !e.name.starts_with(&format!("{}/", c.name)))
                {
                    entries.push(CatalogEntry {
                        kind: EntryKind::Component,
                        name: c.name.clone(),
                        description: c.description.clone(),
                    });
                }
            }
        }

        if entries.is_empty() {
            entries.push(CatalogEntry {
                kind: EntryKind::Component,
                name: "(no components)".into(),
                description: "run `braid setup` to install".into(),
            });
        }

        Self {
            entries,
            cursor: 0,
            filter: String::new(),
        }
    }

    pub fn visible(&self) -> Vec<&CatalogEntry> {
        if self.filter.is_empty() {
            self.entries.iter().collect()
        } else {
            let q = self.filter.to_lowercase();
            self.entries
                .iter()
                .filter(|e| {
                    e.name.to_lowercase().contains(&q) || e.description.to_lowercase().contains(&q)
                })
                .collect()
        }
    }

    pub const fn move_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn move_down(&mut self) {
        let max = self.visible().len().saturating_sub(1);
        if self.cursor < max {
            self.cursor += 1;
        }
    }

    #[allow(dead_code)]
    pub fn selected(&self) -> Option<&CatalogEntry> {
        self.visible().into_iter().nth(self.cursor)
    }
}
