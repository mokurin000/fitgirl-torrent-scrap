use std::{
    error::Error,
    fs,
    num::NonZero,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use fitgirl_torrent_scrap::{
    DECRYPT_WORKERS, FETCH_WORKERS, FilterType, extract_links::download_worker, fetch::fetch_worker,
};
use spdlog::warn;

#[derive(argh::FromArgs)]
#[argh(
    help_triggers("-h", "--help"),
    description = "Scraper for torrents from fitgirl-repacks.site"
)]
struct Args {
    /// scrape from this page num.
    #[argh(option, default = "NonZero::new(1).unwrap()")]
    start_page: NonZero<u16>,

    /// scrape to this page num.
    #[argh(option, default = "NonZero::new(u16::MAX).unwrap()")]
    end_page: NonZero<u16>,

    /// filter scraped contents.
    ///
    /// Avaliable options: adult-only, no-adult, none (default)
    #[argh(option, default = "FilterType::None")]
    filter: FilterType,

    /// directory to save torrents.
    #[argh(option, default = "PathBuf::from(\"./output\")")]
    save_dir: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let Args {
        start_page,
        end_page,
        filter,
        save_dir,
    } = argh::from_env();

    fs::create_dir_all(&save_dir)?;
    nyquest_preset::register();

    let (tx, rx) = kanal::bounded_async(FETCH_WORKERS);
    let (tx_html, rx_html) = kanal::bounded(DECRYPT_WORKERS);
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
        for page in start_page.into()..=end_page.into() {
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
        let tx_html = tx_html.as_async().clone();
        let page_rx = rx.clone();
        let client = client.clone();

        joinset.spawn(fetch_worker(page_rx, client, tx_html));
    }

    for _ in 0..DECRYPT_WORKERS {
        let rx_html = rx_html.clone();
        let is_done = is_done.clone();

        joinset.spawn(download_worker(rx_html, is_done, filter, save_dir.clone()));
    }

    drop(tx_html);
    drop(rx);

    joinset.join_all().await;

    Ok(())
}
