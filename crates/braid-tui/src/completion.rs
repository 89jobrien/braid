use crate::catalog::{Catalog, EntryKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trigger {
    Slash,  // /  → commands & prompts
    At,     // @  → agents
    Dollar, // $  → context vars
}

impl Trigger {
    pub const fn from_char(c: char) -> Option<Self> {
        match c {
            '/' => Some(Self::Slash),
            '@' => Some(Self::At),
            '$' => Some(Self::Dollar),
            _ => None,
        }
    }

    pub const fn sigil(&self) -> char {
        match self {
            Self::Slash => '/',
            Self::At => '@',
            Self::Dollar => '$',
        }
    }

    pub const fn title(&self) -> &'static str {
        match self {
            Self::Slash => "Commands",
            Self::At => "Agents",
            Self::Dollar => "Context",
        }
    }
}

pub struct CompletionState {
    pub trigger: Trigger,
    pub filter: String,
    pub items: Vec<String>,
    pub selected: usize,
}

impl CompletionState {
    pub fn open(trigger: Trigger, catalog: &Catalog) -> Self {
        let items = Self::build_items(&trigger, catalog, "");
        Self {
            trigger,
            filter: String::new(),
            items,
            selected: 0,
        }
    }

    #[allow(dead_code)]
    pub fn push(&mut self, c: char, catalog: &Catalog) {
        self.filter.push(c);
        self.rebuild(catalog);
    }

    #[allow(dead_code)]
    pub fn pop(&mut self, catalog: &Catalog) -> bool {
        if self.filter.is_empty() {
            return false; // signal: close
        }
        self.filter.pop();
        self.rebuild(catalog);
        true
    }

    pub const fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub const fn move_down(&mut self) {
        if self.selected + 1 < self.items.len() {
            self.selected += 1;
        }
    }

    /// Returns the text to insert into the input (including the sigil).
    pub fn accept(&self) -> Option<String> {
        self.items
            .get(self.selected)
            .map(|item| format!("{}{} ", self.trigger.sigil(), item))
    }

    pub fn rebuild(&mut self, catalog: &Catalog) {
        self.items = Self::build_items(&self.trigger, catalog, &self.filter);
        self.selected = self.selected.min(self.items.len().saturating_sub(1));
    }

    fn build_items(trigger: &Trigger, catalog: &Catalog, filter: &str) -> Vec<String> {
        let q = filter.to_lowercase();
        let raw: Vec<String> = match trigger {
            Trigger::Slash => catalog
                .entries
                .iter()
                .filter(|e| {
                    matches!(e.kind, EntryKind::Command | EntryKind::Prompt)
                        && (q.is_empty() || e.name.to_lowercase().contains(&q))
                })
                .map(|e| e.name.clone())
                .collect(),

            Trigger::At => {
                let agents = agents_list();
                agents
                    .into_iter()
                    .filter(|a| q.is_empty() || a.to_lowercase().contains(&q))
                    .collect()
            }

            Trigger::Dollar => {
                let vars = &["session", "doob", "repo", "model", "context"];
                vars.iter()
                    .filter(|v| q.is_empty() || v.contains(q.as_str()))
                    .map(ToString::to_string)
                    .collect()
            }
        };

        if raw.is_empty() { vec![] } else { raw }
    }
}

/// Read agent names from ~/.claude/agents/*.md
fn agents_list() -> Vec<String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let agents_dir = std::path::PathBuf::from(home)
        .join(".claude")
        .join("agents");
    let Ok(entries) = std::fs::read_dir(&agents_dir) else {
        return built_in_agents();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            if p.extension()?.to_str()? == "md" {
                Some(p.file_stem()?.to_string_lossy().into_owned())
            } else {
                None
            }
        })
        .collect();
    names.sort();
    if names.is_empty() {
        built_in_agents()
    } else {
        names
    }
}

fn built_in_agents() -> Vec<String> {
    ["forge", "sentinel", "navigator", "conductor", "herald"]
        .iter()
        .map(ToString::to_string)
        .collect()
}
