use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use kanal::Receiver;
use scraper::Selector;
use tokio::task::spawn_blocking;

use crate::{FilterType, decrypt_torrents::save_torrent_files};

pub async fn download_worker(
    rx_html: Receiver<String>,
    is_done: Arc<AtomicBool>,
    filter: FilterType,
    save_dir: PathBuf,
) {
    while let Ok(text) = rx_html.recv() {
        let clone_is_done = is_done.clone();
        let links = spawn_blocking(move || {
            let html = scraper::Html::parse_document(&text);

            let article_selector = Selector::parse("article").expect("invalid selector");
            let links_selector = Selector::parse("a").expect("invalid selector");
            let page_end_selector = Selector::parse("h1.page-title").expect("invalid selector");

            if html.select(&page_end_selector).next().is_some() {
                clone_is_done.store(true, Ordering::Release);
                return None;
            };

            let articles = html.select(&article_selector);

            let links: Vec<_> = articles
                .map(|article| {
                    let tags =
                        Selector::parse("div.entry-content > p > a").expect("invalid selector");
                    let title =
                        Selector::parse("header > h1.entry-title > a").expect("invalid selector");

                    let is_adult = article.select(&title).next().is_some_and(|title| {
                        title.text().next().is_some_and(|t| t.contains("Adult"))
                    }) || article
                        .select(&tags)
                        .any(|t| t.text().collect::<String>().contains("Adult"));

                    match filter {
                        FilterType::AdultOnly if !is_adult => return vec![],
                        FilterType::NoAdult if is_adult => return vec![],
                        _ => (),
                    }

                    article
                        .select(&links_selector)
                        .filter(|e| e.text().collect::<String>() == ".torrent file only")
                        .filter_map(|e| e.attr("href"))
                        .filter(|s| !s.contains("sendfile.su"))
                        .map(str::to_string)
                        .collect()
                })
                .flatten()
                .collect();

            Some(links)
        })
        .await
        .unwrap();

        let Some(links) = links else { continue };

        save_torrent_files(links, &save_dir).await;

        if is_done.load(Ordering::Acquire) {
            break;
        }
    }
}
