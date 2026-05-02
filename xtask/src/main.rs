//! xtask — build/check automation for the braid workspace.
//!
//! Usage:
//!   cargo xtask check-deps

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

fn main() -> Result<()> {
    let task = std::env::args().nth(1);
    match task.as_deref() {
        Some("check-deps") => check_deps(),
        Some(other) => bail!("unknown task: {other}. Available: check-deps"),
        None => bail!("no task specified. Available: check-deps"),
    }
}

// ---------------------------------------------------------------------------
// Boundary rules
// ---------------------------------------------------------------------------

/// A violation means `dependent` must NOT depend on `forbidden`.
struct Rule {
    /// The crate that has a restriction.
    dependent: &'static str,
    /// Crates that `dependent` must not (transitively) declare as a direct dep.
    forbidden: &'static [&'static str],
    /// Human-readable reason shown on violation.
    reason: &'static str,
}

/// All boundary rules for the braid hexagonal architecture.
///
/// Chain:  braid-cli → braid-providers → braid-core → braid-ports → braid-model
///
/// "Support" crates (redact, hooks, mcp, observe, context, tui, bootstrap) may
/// only depend on crates at `braid-core` or below in the chain — never on
/// `braid-providers` or `braid-cli`.
const RULES: &[Rule] = &[
    // --- Main chain: no upward dependencies ---
    Rule {
        dependent: "braid-model",
        forbidden: &["braid-ports", "braid-core", "braid-providers", "braid-cli"],
        reason: "braid-model is the leaf; it must not depend on any other braid crate",
    },
    Rule {
        dependent: "braid-ports",
        forbidden: &["braid-core", "braid-providers", "braid-cli"],
        reason: "braid-ports sits below braid-core in the chain",
    },
    Rule {
        dependent: "braid-core",
        forbidden: &["braid-providers", "braid-cli"],
        reason: "braid-core must not depend on braid-providers or braid-cli",
    },
    Rule {
        dependent: "braid-providers",
        forbidden: &["braid-cli"],
        reason: "braid-providers must not depend on braid-cli",
    },
    // --- Support crates: must not touch providers or cli ---
    Rule {
        dependent: "braid-redact",
        forbidden: &["braid-providers", "braid-cli"],
        reason: "support crate braid-redact must only depend on braid-core/braid-ports/braid-model",
    },
    Rule {
        dependent: "braid-hooks",
        forbidden: &["braid-providers", "braid-cli"],
        reason: "support crate braid-hooks must only depend on braid-core/braid-ports/braid-model",
    },
    Rule {
        dependent: "braid-mcp",
        forbidden: &["braid-providers", "braid-cli"],
        reason: "support crate braid-mcp must only depend on braid-core/braid-ports/braid-model",
    },
    Rule {
        dependent: "braid-observe",
        forbidden: &["braid-providers", "braid-cli"],
        reason: "support crate braid-observe must only depend on braid-core/braid-ports/braid-model",
    },
    Rule {
        dependent: "braid-context",
        forbidden: &["braid-providers", "braid-cli"],
        reason: "support crate braid-context must only depend on braid-core/braid-ports/braid-model",
    },
    Rule {
        dependent: "braid-tui",
        forbidden: &["braid-providers", "braid-cli"],
        reason: "support crate braid-tui must only depend on braid-core/braid-ports/braid-model",
    },
    Rule {
        dependent: "braid-bootstrap",
        forbidden: &["braid-providers", "braid-cli"],
        reason: "support crate braid-bootstrap must only depend on braid-core/braid-ports/braid-model",
    },
];

// ---------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------

fn check_deps() -> Result<()> {
    let workspace_root = workspace_root()?;
    let crates_dir = workspace_root.join("crates");

    // Build a map: crate-name → direct braid-* dependencies.
    let dep_map = build_dep_map(&crates_dir)?;

    let mut violations: Vec<String> = Vec::new();

    for rule in RULES {
        let Some(deps) = dep_map.get(rule.dependent) else {
            // Crate not present — skip (it may not exist yet).
            continue;
        };

        for &forbidden in rule.forbidden {
            if deps.contains(forbidden) {
                violations.push(format!(
                    "  VIOLATION: {} depends on {} — {}",
                    rule.dependent, forbidden, rule.reason
                ));
            }
        }
    }

    if violations.is_empty() {
        println!("check-deps: all architecture boundary rules pass.");
        Ok(())
    } else {
        eprintln!("check-deps: architecture boundary violations found:\n");
        for v in &violations {
            eprintln!("{v}");
        }
        eprintln!();
        bail!("{} violation(s) — see output above", violations.len())
    }
}

/// Returns the workspace root by walking up from the xtask binary location.
fn workspace_root() -> Result<PathBuf> {
    // When run via `cargo xtask`, the CWD is the workspace root.
    let cwd = std::env::current_dir().context("cannot determine current directory")?;
    // Verify it looks right by checking for Cargo.toml with [workspace].
    let cargo_toml = cwd.join("Cargo.toml");
    if !cargo_toml.exists() {
        bail!(
            "expected workspace Cargo.toml at {}, got nothing — run from workspace root",
            cargo_toml.display()
        );
    }
    Ok(cwd)
}

/// Reads every `crates/<name>/Cargo.toml` and extracts direct `braid-*` deps.
fn build_dep_map(crates_dir: &Path) -> Result<HashMap<String, HashSet<String>>> {
    let mut map = HashMap::new();

    let entries = std::fs::read_dir(crates_dir)
        .with_context(|| format!("cannot read crates dir: {}", crates_dir.display()))?;

    for entry in entries {
        let entry = entry.context("directory entry error")?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let cargo_toml_path = path.join("Cargo.toml");
        if !cargo_toml_path.exists() {
            continue;
        }

        let crate_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(String::from)
            .context("crate directory has non-UTF-8 name")?;

        let deps = parse_braid_deps(&cargo_toml_path)
            .with_context(|| format!("parsing {}", cargo_toml_path.display()))?;

        map.insert(crate_name, deps);
    }

    Ok(map)
}

/// Parses a `Cargo.toml` and returns all keys that start with `braid-` found
/// in `[dependencies]` and `[dev-dependencies]`.
fn parse_braid_deps(cargo_toml: &Path) -> Result<HashSet<String>> {
    let content = std::fs::read_to_string(cargo_toml)
        .with_context(|| format!("cannot read {}", cargo_toml.display()))?;

    let doc: toml::Value = content
        .parse()
        .with_context(|| format!("invalid TOML in {}", cargo_toml.display()))?;

    let mut braid_deps = HashSet::new();

    for section in &["dependencies", "dev-dependencies"] {
        if let Some(toml::Value::Table(deps)) = doc.get(section) {
            for key in deps.keys() {
                if key.starts_with("braid-") {
                    braid_deps.insert(key.clone());
                }
            }
        }
    }

    Ok(braid_deps)
}
