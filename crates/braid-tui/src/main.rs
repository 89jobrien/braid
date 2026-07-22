#![cfg_attr(test, allow(clippy::unwrap_used))]
mod catalog;
mod chat;
mod completion;
mod keys;
mod model;
mod ui;

use std::sync::Arc;

use anyhow::{Context, Result};
use braid_observe::SessionStore;
use crossterm::event::{self, Event as CrossEvent};
use ratatui_tea::Program;

use crate::model::{AppModel, Msg};

fn default_store_dir() -> Result<std::path::PathBuf> {
    let home = std::env::var("HOME").context("HOME not set")?;
    Ok(std::path::PathBuf::from(home)
        .join(".braid")
        .join("sessions"))
}

fn main() -> Result<()> {
    let store_dir = default_store_dir()?;
    let store = Arc::new(SessionStore::open(store_dir)?);
    let model_name = std::env::var("BRAID_MODEL").unwrap_or_else(|_| "gpt-oss".to_string());

    let app_model = AppModel::new(Arc::clone(&store), model_name)?;

    // Install panic hook that restores terminal before printing the panic message.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        ratatui::restore();
        default_hook(info);
    }));

    let mut terminal = ratatui::init();
    let mut program = Program::new(app_model);
    program.init();

    loop {
        program.draw(&mut terminal)?;

        // Poll the engine thread for replies
        if let Some(reply) = program.model_mut().poll_engine() {
            program.send(Msg::EngineReply(reply));
        }

        if !event::poll(std::time::Duration::from_millis(100))? {
            continue;
        }

        let CrossEvent::Key(key) = event::read()? else {
            continue;
        };

        program.send(Msg::Key(key));

        if program.model().should_quit {
            break;
        }
    }

    ratatui::restore();
    Ok(())
}
