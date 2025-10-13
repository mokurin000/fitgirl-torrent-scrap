use nyquest::AsyncClient;
use std::{error::Error, path::Path, time::UNIX_EPOCH};
use tokio::{fs, task};

use chrono::NaiveDate;
use fitgirl_decrypt::{Attachment, Paste, base64::Engine as _, decrypt_with_key};
use spdlog::{error, info};

use crate::Game;
use db_helper::{add_game, query_game, read_transac, write_transac};

pub(crate) async fn save_torrent_files(
    games: Vec<Game>,
    save_dir: impl AsRef<Path>,
    client: &AsyncClient,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let save_dir = save_dir.as_ref();
    let mut filtered_games = vec![];
    {
        let tsx = read_transac()?;
        for game in games {
            let torrent = query_game(&tsx, &game.title)?;
            match torrent {
                Some(torrent_name) => {
                    if save_dir.join(torrent_name).metadata().is_ok_and(|meta| {
                        meta.is_file()
                            && meta.modified().is_ok_and(|time| {
                                let Ok(time) = time.duration_since(UNIX_EPOCH) else {
                                    return false;
                                };
                                let days = time.as_secs() / (24 * 60 * 60);
                                NaiveDate::from_epoch_days(days as _)
                                    .is_some_and(|store_date| game.date <= store_date)
                            })
                    }) {
                        continue;
                    }
                }
                None => (),
            }

            filtered_games.push(game);
        }
    }

    let tsx = write_transac()?;
    for (
        paste,
        Game {
            paste_url: url,
            title,
            ..
        },
    ) in filtered_games
        .iter()
        .filter_map(|g| Paste::parse_url(&g.paste_url).ok().map(|paste| (paste, g)))
    {
        let Ok(cipher) = paste
            .request_async_ny(client.clone())
            .await
            .inspect_err(|e| error!("{url}: {e}"))
        else {
            continue;
        };

        let master_key = paste.master_key().clone();
        match task::spawn_blocking(move || decrypt_with_key(&master_key, cipher)).await? {
            Ok(Attachment {
                attachment,
                attachment_name,
            }) => {
                add_game(&tsx, title, &attachment_name)?;

                let output = save_dir.join(&attachment_name);

                let Some(torrent) = attachment
                    .strip_prefix("data:application/x-bittorrent;base64,")
                    .and_then(|b| {
                        fitgirl_decrypt::base64::prelude::BASE64_STANDARD
                            .decode(b)
                            .ok()
                    })
                else {
                    continue;
                };

                let _ = fs::write(output, torrent).await;
                info!("saved {attachment_name}");
            }
            Err(fitgirl_decrypt::Error::JSONSerialize(_)) => {
                error!("{url}: attachment is missing");
            }
            Err(e) => {
                error!("{url}: {e}");
            }
        }
    }

    tsx.commit()?;

    Ok(())
}
