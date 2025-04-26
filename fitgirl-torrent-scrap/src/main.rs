use std::{
    error::Error,
    fs,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use tracing::{level_filters::LevelFilter, warn};
use tracing_subscriber::EnvFilter;

use fitgirl_torrent_scrap::{
    DECRYPT_WORKERS, FETCH_WORKERS, OUTPUT_DIR, extract_links::download_worker, fetch::fetch_worker,
};

#[derive(argh::FromArgs)]
#[argh(
    help_triggers("-h", "--help"),
    description = "Scraper for torrents from fitgirl-repacks.site"
)]
struct Args {
    /// scrape from this page num. page 0 is treated as page 1.
    #[argh(option, default = "1")]
    start_page: u16,

    /// scrape to this page num.
    #[argh(option, default = "u16::MAX")]
    end_page: u16,

    /// skip adult contents
    #[argh(switch)]
    skip_adult: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let Args {
        start_page,
        end_page,
        skip_adult,
    } = argh::from_env();

    fs::create_dir_all(OUTPUT_DIR)?;
    nyquest_preset::register();

    tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();

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
        let tx_html = tx_html.as_async().clone();
        let page_rx = rx.clone();
        let client = client.clone();

        joinset.spawn(fetch_worker(page_rx, client, tx_html));
    }

    for _ in 0..DECRYPT_WORKERS {
        let rx_html = rx_html.clone();
        let is_done = is_done.clone();

        joinset.spawn(download_worker(rx_html, is_done, skip_adult));
    }

    drop(tx_html);
    drop(rx);

    joinset.join_all().await;

    Ok(())
}
