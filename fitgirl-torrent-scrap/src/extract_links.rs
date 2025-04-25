use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use kanal::Receiver;
use scraper::Selector;
use tokio::task::spawn_blocking;

use crate::decrypt_torrents::save_torrent_files;

pub async fn download_worker(rx_html: Receiver<String>, is_done: Arc<AtomicBool>) {
    while let Ok(text) = rx_html.recv() {
        let clone_is_done = is_done.clone();
        let links = spawn_blocking(move || {
            let html = scraper::Html::parse_document(&text);

            let links_selector = Selector::parse("a").expect("invalid selector");
            let page_end_selector = Selector::parse("h1.page-title").expect("invalid selector");

            if html.select(&page_end_selector).next().is_some() {
                clone_is_done.store(true, Ordering::Release);
                return None;
            };

            let links: Vec<_> = html
                .select(&links_selector)
                .filter(|e| e.text().collect::<String>() == ".torrent file only")
                .filter_map(|e| e.attr("href"))
                .filter(|s| !s.contains("sendfile.su"))
                .map(str::to_string)
                .collect();

            Some(links)
        })
        .await
        .unwrap();

        let Some(links) = links else { continue };

        save_torrent_files(links).await;

        if is_done.load(Ordering::Acquire) {
            break;
        }
    }
}
