use anyhow::Result;

mod app;
mod audio;
mod browser;
mod chapters;
mod config;
mod library;
mod list;
mod metadata;
mod ui;

fn main() -> Result<()> {
    let cfg = config::load()?;
    let mut app = app::App::new(cfg)?;
    app.run()
}
