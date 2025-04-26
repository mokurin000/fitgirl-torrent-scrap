use std::{fs, path::Path};

use fitgirl_decrypt::{Attachment, Paste, base64::Engine as _};
use tracing::{debug, error, info};

pub(crate) async fn save_torrent_files(links: Vec<String>, save_dir: impl AsRef<Path>) {
    for (paste, url) in links
        .iter()
        .filter_map(|url| Paste::parse_url(url).ok().map(|paste| (paste, url)))
    {
        let Ok(cipher) = paste
            .request_async()
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
                let output = save_dir.as_ref().join(&attachment_name);
                if output.exists() {
                    debug!("skipped existing {}", output.to_string_lossy());
                    continue;
                }
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
}
