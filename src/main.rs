use std::{
    error::Error,
    fs,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU16},
    },
};

use fitgirl_decrypt::{Attachment, Paste, base64::Engine};
use scraper::Selector;

const FETCH_WORKERS: usize = 5;
const DECRYPT_WORKERS: usize = 5;
const OUTPUT_DIR: &str = "./output/";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    fs::create_dir_all(OUTPUT_DIR)?;

    let (tx, rx) = kanal::bounded_async(FETCH_WORKERS);
    let (tx_text, rx_text) = kanal::bounded(DECRYPT_WORKERS);
    let is_done = Arc::new(AtomicBool::new(false));
    let current_page = Arc::new(AtomicU16::new(1));

    let _is_done = is_done.clone();
    let _current_page = current_page.clone();
    ctrlc::set_handler(move || {
        println!(
            "current_page: {}",
            _current_page.load(std::sync::atomic::Ordering::Acquire)
        );
        _is_done.store(true, std::sync::atomic::Ordering::Release);
    })?;

    let _is_done = is_done.clone();
    let _current_page = current_page.clone();
    tokio::spawn(async move {
        for i in 1..=u16::MAX {
            if _is_done.load(std::sync::atomic::Ordering::Acquire) {
                break;
            }
            _current_page.store(i, std::sync::atomic::Ordering::Release);
            let _ = tx.send(i).await;
        }
    });

    let client = reqwest::ClientBuilder::new().build()?;
    let mut joinset = tokio::task::JoinSet::new();

    for _ in 0..FETCH_WORKERS {
        let tx_text = tx_text.as_async().clone();
        let rx = rx.clone();
        let client = client.clone();

        joinset.spawn(async move {
            while let Ok(page) = rx.recv().await {
                let url = format!("https://fitgirl-repacks.site/page/{page}/");
                if let Ok(resp) = client.get(url).send().await {
                    let text = resp.text().await.unwrap();
                    let _ = tx_text.send(text).await;
                }
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
                    _is_done.store(true, std::sync::atomic::Ordering::Release);
                    return;
                };

                for paste in html
                    .select(&links_selector)
                    .filter(|e| e.text().collect::<String>() == ".torrent file only")
                    .filter_map(|e| e.attr("href"))
                    .filter_map(|url| Paste::parse_url(url).ok())
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
                        Err(e) => {
                            eprintln!("decrypt error: {e}");
                        }
                    }
                }
            }
        });
    }

    drop(tx_text);
    drop(rx);

    joinset.join_all().await;

    Ok(())
}
