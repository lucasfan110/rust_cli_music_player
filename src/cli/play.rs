use std::{
  cell::Cell,
  process,
  sync::{mpsc, PoisonError},
  thread,
  time::Duration,
};

use anyhow::Context;
use basic_quick_lib::{cli_util::pause, io_util::input_trim};
use colored::Colorize;
use log::{error, info, warn};
use rand::{prelude::SliceRandom, thread_rng};
use soloud::{AudioExt, LoadExt, Soloud, Wav};

use crate::{
  cli::data::Song,
  util::settings::{PlaybackMode, SETTINGS},
};

use super::data::PlaylistInfo;

fn shuffle_vec(vec: &mut Vec<usize>, len: usize) {
  *vec = (0..len).collect();
  vec.shuffle(&mut thread_rng());
}

#[derive(thiserror::Error, Debug)]
enum GetIndexError {
  #[error("Index cannot be zero!")]
  IndexIsZero,

  #[error("{0} is not a valid index!")]
  InvalidIndex(i32),
}

/// get the index (positive or negative, but can't be zero) and
/// return the actual index.
/// Basically used for converting negative index
fn get_index(index: i32, vec_len: usize) -> Result<usize, GetIndexError> {
  use GetIndexError::*;

  let vec_index = match index {
    0 => return Err(IndexIsZero),
    1..=i32::MAX => index - 1,
    i32::MIN..=-1 => vec_len as i32 + index,
  };

  if vec_index.is_negative() || vec_index >= vec_len as i32 {
    return Err(InvalidIndex(index));
  }

  Ok(vec_index as usize)
}

fn option_print(command: &str, help_msg: &str) {
  info!("{:<20} --- {}", command, help_msg);
}

#[derive(clap::Args)]
pub struct Play {
  playlist_name: String,
}

impl Play {
  pub fn handle(&self) {
    let playlist_info = match PlaylistInfo::load(&self.playlist_name) {
      Ok(v) => v,
      Err(err) => {
        error!(
          r#"Failed to load playlist "{}"! Error: {}"#,
          self.playlist_name, err
        );
        return;
      }
    };
    self.play(playlist_info);
  }

  fn play(&self, playlist_info: PlaylistInfo) {
    use Message::*;

    let PlaylistInfo {
      name,
      songs,
      // created,
      ..
    } = playlist_info;

    let (sender, receiver) = mpsc::channel::<Message>();

    let songs_len = songs.len();

    thread::spawn(move || {
      if songs_len == 0 {
        warn!("The playlist is empty! Use command `add <LINK> <PLAYLIST_NAME>` to add songs into playlist!");
        pause();
        process::exit(1);
      }

      let mut sl = Soloud::default()
        .with_context(|| "Failed to get player!")
        .unwrap();

      let mut wav = Wav::default();

      let volume = SETTINGS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .volume;

      sl.set_global_volume(volume as f32 / 100.0);

      let mut randomized_indexes = Vec::new();
      shuffle_vec(&mut randomized_indexes, songs_len);

      let currently_playing = {
        let starting_index = if SETTINGS
          .lock()
          .unwrap_or_else(PoisonError::into_inner)
          .playback_mode
          == PlaybackMode::Random
        {
          randomized_indexes.pop().unwrap()
        } else {
          0
        };

        Cell::new(starting_index)
      };

      'song_loop: loop {
        let Song {
          song_name,
          path_to_song,
          // author,
          ..
        } = &songs[currently_playing.get()];

        let mut current_duration = Duration::ZERO;
        // use a cell here because we want the `print_info` closure
        // to be dynamically reflected and because of rust's ownership
        // system does not allow this then cell is used instead
        let is_paused = Cell::new(false);

        let print_info = || {
          // just ignore it if failed to clear
          clearscreen::clear().unwrap_or_default();

          info!("Playlist: {}", name);
          // info!(
          //   "Created at {}",
          //   created
          //     .as_ref()
          //     .map(|v| v.to_string())
          //     .unwrap_or_else(|| "Unknown".to_string())
          // );

          println!();

          for (index, song) in songs.iter().enumerate() {
            let is_bold = index == currently_playing.get();

            if is_bold {
              info!("{}", format!("{}. {}", index + 1, song.song_name).bold())
            } else {
              info!("{}", format!("{}. {}", index + 1, song.song_name))
            }
          }

          if is_paused.get() {
            info!("Paused");
          }
        };

        print_info();

        if let Err(e) = wav.load(path_to_song) {
          error!("Failed to load song \"{}\"! Error: {}", song_name, e);
          info!(
						"The reason why it failed might be because {} does not exists or is an audio type that is not supported", 
						path_to_song.to_str().unwrap_or("Unknown Path")
					);
          pause();
          continue;
        };

        let handle = sl.play(&wav);

        const SLEEP_DURATION: Duration = Duration::from_millis(10);

        // while the song is playing
        while sl.voice_count() > 0 {
          // pause the loop a little so it won't take too much cpu power
          thread::sleep(SLEEP_DURATION);

          if !is_paused.get() {
            current_duration += SLEEP_DURATION;
          }

          // try to recv to see if there is any command, or else continue playing the song
          if let Ok(message) = receiver.try_recv() {
            match message {
              Pause => {
                is_paused.set(true);
                sl.set_pause(handle, is_paused.get());
              }
              Resume => {
                is_paused.set(false);
                sl.set_pause(handle, is_paused.get());
              }
              PauseOrResume => {
                // inverse bool but I'm too lazy
                is_paused.set(!is_paused.get());
                sl.set_pause(handle, is_paused.get());
              }
              // do nothing because it will reprint anyways lol it feels stupid
              Reprint => {}
              IndexJump(index) => {
                currently_playing.set(index);
                continue 'song_loop;
              }
              SetVolume(new_volume) => {
                sl.set_global_volume(new_volume as f32 / 100.0);
                let mut setting =
                  SETTINGS.lock().unwrap_or_else(PoisonError::into_inner);
                setting.volume = new_volume;
                let _ = setting.save();
              }
            }

            print_info();
          }
        }

        let setting = SETTINGS.lock().unwrap_or_else(PoisonError::into_inner);

        // after the song been played, change the current playing song
        // based on the playback mode choice
        match setting.playback_mode {
          PlaybackMode::Sequel => {
            currently_playing.set(currently_playing.get() + 1);
            if currently_playing.get() >= songs.len() {
              break;
            }
          }
          PlaybackMode::LoopOnce => {}
          PlaybackMode::LoopPlaylist => {
            currently_playing.set(currently_playing.get() + 1);
            if currently_playing.get() >= songs.len() {
              currently_playing.set(0);
            }
          }
          PlaybackMode::Random => currently_playing.set(
            randomized_indexes.pop().unwrap_or_else(|| {
              shuffle_vec(&mut randomized_indexes, songs_len);

              // should not panic because if the playlist is empty then
              // it will be check and will exit the process if so
              randomized_indexes.pop().unwrap()
            }),
          ),
        }
      }

      process::exit(0);
    });

    let try_send = |message: Message| {
      sender.send(message).unwrap_or_else(|err| {
				error!("Something went wrong while sending the command to the music playing thread! This command will not do anything! Error: {}", err);
			});
    };

    loop {
      try_send(Reprint);

      let input = input_trim("");

      if input.is_empty() {
        continue;
      }

      let splitted = input.split(' ').collect::<Vec<_>>();

      let command = &splitted[0];
      let args = &splitted[1..];

      match *command {
        "exit" => return,
        "setv" => {
          if args.is_empty() {
            warn!("usage: setv <VOLUME>");
            pause();
            continue;
          }

          let volume = match args[0].trim().parse::<u64>() {
            Ok(v) => v,
            Err(err) => {
              warn!("Failed to parse {}. Err: {}", args[0].trim(), err);
              pause();
              continue;
            }
          };

          if !(0..=100).contains(&volume) {
            warn!("Volume must be inbetween 0 and 100!");
            pause();
            continue;
          }

          try_send(SetVolume(volume as u8));
        }
        "help" => {
          option_print("exit", "exit the program");
          option_print("help", "open help message");
          option_print("pause", "pause the music");
          option_print("resume", "resume the music");
          option_print(
            "pr",
            "pause the music if it is playing otherwise resume",
          );
          option_print("setv <VOLUME>", "set the volume. anything that is not inbetween 0 and 100 will be invalid");
          info!("Type the index of the song to jump to the song. Example: `4` will jump to the fourth one");
          info!("    - Note that you can pass a negative value to start from the back. Example `-1` will go to the last song");
          pause();
          try_send(Reprint);
        }
        "pause" => {
          try_send(Pause);
        }
        "resume" => {
          try_send(Resume);
        }
        "pr" => {
          try_send(PauseOrResume);
        }
        num if num.parse::<i32>().is_ok() => {
          // a really dumb thing to do and hopefully
          // if let guard can be stabilized in the future
          let num = num.parse::<i32>().unwrap();

          let index = match get_index(num, songs_len) {
            Ok(index) => index,
            Err(e) => {
              error!("{}", e);
              pause();
              try_send(Reprint);
              continue;
            }
          };

          try_send(IndexJump(index));
        }
        _ => {
          warn!(
            "Unknown Command `{}`! Type in `help` for more information!",
            input
          );
          pause();
        }
      }
    }
  }
}

enum Message {
  Pause,
  Resume,
  PauseOrResume,
  Reprint,
  IndexJump(usize),
  SetVolume(u8),
}

#[cfg(test)]
mod test {
  use soloud::{AudioExt, LoadExt, Soloud, Wav};

  #[test]
  fn voice_count() -> anyhow::Result<()> {
    let sl = Soloud::default()?;

    let mut wav = Wav::default();
    wav.load("JJD - Adventure [NCS Release].wav")?;

    sl.play(&wav);

    println!("active voice count: {}", wav.length());

    Ok(())
  }
}
