use std::{num::NonZeroUsize, process};

use basic_quick_lib::io_util::input_other_repeat;
use termcolor::ColorSpec;

use crate::util::{add_from_youtube_link, colored, decode_html_entities, youtube_api};

#[derive(clap::Args)]
pub struct Search {
    /// The search term
    query: String,

    /// The playlist which the music is going to be added
    #[clap(short, long)]
    add_to: String,
}

impl Search {
    pub fn handle(&self) {
        let result = match youtube_api::search(&self.query, 10) {
            Ok(r) => r,
            Err(e) => {
                println!("Cannot search youtube. Error: {}", e);
                process::exit(1);
            }
        };

        let videos = result["items"].as_array().unwrap();

        for (index, video) in videos.iter().enumerate() {
            let channel_info = format!(
                " - {}",
                decode_html_entities(video["snippet"]["channelTitle"].as_str().unwrap())
            );

            print!(
                "{}. {}{}",
                index + 1,
                decode_html_entities(video["snippet"]["title"].as_str().unwrap()),
                channel_info
            );

            colored::writeln(ColorSpec::new().set_bold(true), &channel_info);
        }

        let input: NonZeroUsize = input_other_repeat("Type which one to download: ");
        let selected_video = &videos[input.get() - 1];
        let video_id = selected_video["id"]["videoId"].as_str().unwrap();

        // TODO: Download video & add to playlist
        let link = format!("https://www.youtube.com/watch?v={video_id}");
        if let Err(e) = add_from_youtube_link(&self.add_to, &link) {
            println!("Failed to download video. Error: {e}");
        }
    }
}
