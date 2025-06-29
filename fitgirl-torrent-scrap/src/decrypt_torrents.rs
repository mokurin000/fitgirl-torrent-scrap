use std::{error::Error, fs, path::Path};

use fitgirl_decrypt::{Attachment, Paste, base64::Engine as _};
use spdlog::{debug, error, info};

use crate::{
    Game,
};
use db_helper::{add_game, query_game, read_transac, write_transac};

pub(crate) async fn save_torrent_files(
    games: Vec<Game>,
    save_dir: impl AsRef<Path>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let save_dir = save_dir.as_ref();
    let mut filtered_games = vec![];
    {
        let tsx = read_transac()?;
        for game in games {
            if query_game(&tsx, &game.title)?
                .is_some_and(|torrent_name| save_dir.join(torrent_name).exists())
            {
                continue;
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
        },
    ) in filtered_games
        .iter()
        .filter_map(|g| Paste::parse_url(&g.paste_url).ok().map(|paste| (paste, g)))
    {
        let Ok(cipher) = paste
            .request_async_ny()
            .await
            .inspect_err(|e| error!("{url}: {e}"))
        else {
            continue;
        };

        match paste.decrypt(cipher) {
            Ok(Attachment {
                attachment,
                attachment_name,
            }) => {
                add_game(&tsx, title, &attachment_name)?;

                let output = save_dir.join(&attachment_name);
                if output.exists() {
                    debug!("skipped existing {}", output.to_string_lossy());
                    continue;
                }

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

                let _ = fs::write(output, torrent);
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
