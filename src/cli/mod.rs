use self::{
    add::Add, create::Create, delete::Delete, play::Play, rename::Rename, search::Search,
    settings::ChangeSettings,
};
use clap::Parser;

mod add;
mod create;
pub mod data;
mod delete;
mod play;
mod rename;
mod search;
mod settings;

#[derive(Parser)]
#[clap(
    author,
    version,
    about,
    long_about = "CLI music player because I'm too lazy to make a ui one lol"
)]
pub struct Cli {
    #[clap(subcommand)]
    commands: Subcommands,
}

impl Cli {
    pub fn handle(&mut self) {
        use Subcommands::*;

        match &mut self.commands {
            Play(play) => play.handle(),
            Add(add) => add.handle(),
            Setting(change_settings) => change_settings.handle(),
            Search(search) => search.handle(),
            Create(create) => create.handle(),
            Rename(rename) => rename.handle(),
            Delete(delete) => delete.handle(),
        }
    }
}

#[derive(clap::Subcommand)]
enum Subcommands {
    /// Play a playlist
    Play(Play),

    /// Add a music to a playlist. Can either be a youtube link or a local path
    Add(Add),

    /// Change the settings of the music player
    Setting(ChangeSettings),

    /// Search for music on youtube and download them
    Search(Search),

    /// Create a new playlist
    Create(Create),

    /// Rename a playlist
    Rename(Rename),

    /// Delete a playlist
    Delete(Delete),
}
