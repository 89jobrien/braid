mod app;
mod keys;
mod ui;

use anyhow::{Context, Result};
use braid_observe::SessionStore;

fn default_store_dir() -> Result<std::path::PathBuf> {
    let home = std::env::var("HOME").context("HOME not set")?;
    Ok(std::path::PathBuf::from(home)
        .join(".braid")
        .join("sessions"))
}

fn main() -> Result<()> {
    let store_dir = default_store_dir()?;
    let store = SessionStore::open(store_dir)?;

    let mut terminal = ratatui::init();
    let result = app::run(&mut terminal, &store);
    ratatui::restore();

    result
}
