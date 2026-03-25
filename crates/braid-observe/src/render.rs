use std::io::Write;

use anyhow::Result;
use braid_model::{Event, EventKind};

use crate::store::SessionMeta;

pub fn render_session(
    events: &[Event],
    meta: Option<&SessionMeta>,
    out: &mut impl Write,
) -> Result<()> {
    todo!()
}
