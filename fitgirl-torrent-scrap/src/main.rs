use std::{
    error::Error,
    fs,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use fitgirl_decrypt::{Attachment, Paste, base64::Engine};
use nyquest::Request;
use scraper::Selector;
use tracing_subscriber::EnvFilter;

use tracing::{error, info, level_filters::LevelFilter, warn};

const FETCH_WORKERS: usize = 5;
const DECRYPT_WORKERS: usize = 10;
const OUTPUT_DIR: &str = "./output/";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    fs::create_dir_all(OUTPUT_DIR)?;
    nyquest_preset::register();

    tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();

    let start_page = std::env::args().nth(1).as_deref().unwrap_or("1").parse()?;
    let end_page: u16 = std::env::args()
        .nth(2)
        .unwrap_or(u16::MAX.to_string())
        .parse()?;

    let (tx, rx) = kanal::bounded_async(FETCH_WORKERS);
    let (tx_text, rx_text) = kanal::bounded(DECRYPT_WORKERS);
    let is_done = Arc::new(AtomicBool::new(false));

    let _is_done = is_done.clone();
    ctrlc::set_handler(move || {
        if _is_done.load(Ordering::Acquire) {
            warn!("ctrl-c twice, force-exiting");
            std::process::exit(0);
        }

        _is_done.store(true, Ordering::Release);
    })?;

    let _is_done = is_done.clone();
    tokio::spawn(async move {
        for page in start_page..=end_page {
            if _is_done.load(Ordering::Acquire) {
                let _ = tx.close();
                break;
            }
            let _ = tx.send(page).await;
        }
    });

    let client = nyquest::ClientBuilder::default()
        .base_url("https://fitgirl-repacks.site")
        .build_async()
        .await?;
    let mut joinset = tokio::task::JoinSet::new();

    for _ in 0..FETCH_WORKERS {
        let tx_text = tx_text.as_async().clone();
        let rx = rx.clone();
        let client = client.clone();
        let _is_done = is_done.clone();

        joinset.spawn(async move {
            while let Ok(page) = rx.recv().await {
                let url = format!("/page/{page}/");
                let Ok(resp) = client.request(Request::get(url)).await else {
                    continue;
                };
                let Ok(text) = resp.text().await else {
                    continue;
                };

                let _ = tx_text.send(text).await;
                info!("processed {page}");
            }
        });
    }

    for _ in 0..DECRYPT_WORKERS {
        let rx_text = rx_text.clone();
        let _is_done = is_done.clone();

        joinset.spawn_blocking(move || {
            while let Ok(text) = rx_text.recv() {
                let html = scraper::Html::parse_document(&text);

                let links_selector = Selector::parse("a").expect("invalid selector");
                let page_end_selector = Selector::parse("h1.page-title").expect("invalid selector");

                if html.select(&page_end_selector).next().is_some() {
                    _is_done.store(true, Ordering::Release);
                    break;
                };

                for (paste, url) in html
                    .select(&links_selector)
                    .filter(|e| e.text().collect::<String>() == ".torrent file only")
                    .filter_map(|e| e.attr("href"))
                    .filter_map(|url| Paste::parse_url(url).ok().map(|paste| (paste, url)))
                {
                    match paste.decrypt() {
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
                            let output = Path::new(OUTPUT_DIR).join(attachment_name);
                            let _ = fs::write(output, torrent);
                        }
                        Err(fitgirl_decrypt::Error::JSONSerialize(_)) => {
                            error!("{url}: attachment is missing");
                        }
                        Err(e) => {
                            error!("{url}: {e}");
                        }
                    }
                }

                if _is_done.load(Ordering::Acquire) {
                    break;
                }
            }
        });
    }

    drop(tx_text);
    drop(rx);

    let _is_done = is_done.clone();
    let wait = async move {
        loop {
            tokio::time::sleep(Duration::from_millis(1000)).await;
            if is_done.load(Ordering::Acquire) {
                break;
            }
        }
    };

    tokio::select! {
        _ = joinset.join_all() => {}
        _ = wait => {}
    }

    Ok(())
}
