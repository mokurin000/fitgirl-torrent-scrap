use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use chrono::NaiveDate;
use kanal::AsyncReceiver;
use nyquest::AsyncClient;
use scraper::{Html, Selector};
use spdlog::{debug, error};
use tokio::task;

use crate::{FilterType, Game, decrypt_torrents::save_torrent_files};

pub async fn download_worker(
    rx_html: AsyncReceiver<Html>,
    is_done: Arc<AtomicBool>,
    filter: FilterType,
    save_dir: PathBuf,
    client: AsyncClient,
) {
    while let Ok(html) = rx_html.recv().await {
        let links = task::spawn_blocking(move || {
            let article_selector = Selector::parse("article").expect("invalid selector");
            let links_selector = Selector::parse("a").expect("invalid selector");

            let articles = html.select(&article_selector);

            let links: Vec<_> = articles
                .filter_map(|article| {
                    let tags = Selector::parse("div.entry-content p > a:not(:first-child)")
                        .expect("invalid selector");
                    let title =
                        Selector::parse("header > h1.entry-title > a").expect("invalid selector");
                    let title = article
                        .select(&title)
                        .next()
                        .map(|t| t.text().next())
                        .flatten()?;
                    let date = article
                        .select(
                            &Selector::parse(".entry-header > .entry-meta > .entry-date time")
                                .expect("invalid selector"),
                        )
                        .next()
                        .and_then(|elem| elem.text().next())?;
                    let date = NaiveDate::parse_from_str(date, "%d/%m/%Y").ok()?;

                    let is_adult = title.to_lowercase().contains("adult")
                        || article
                            .select(&tags)
                            .any(|t| t.text().collect::<String>().contains("Adult"));

                    match filter {
                        FilterType::AdultOnly if !is_adult => return None,
                        FilterType::NoAdult if is_adult => return None,
                        _ => (),
                    }

                    article
                        .select(&links_selector)
                        .filter(|e| e.text().collect::<String>() == ".torrent file only")
                        .filter_map(|e| e.attr("href"))
                        .filter(|s| !s.contains("sendfile.su"))
                        .filter(|s| !s.contains("announce"))
                        .map(str::to_string)
                        .next()
                        .map(|paste_url| Game {
                            paste_url,
                            title: title.into(),
                            date,
                        })
                        .inspect(|game| debug!("scraped: {game:?}"))
                })
                .collect();

            Some(links)
        })
        .await
        .unwrap();

        let Some(links) = links else { continue };

        if let Err(e) = save_torrent_files(links, &save_dir, &client, &is_done).await {
            error!("failed to save torrent: {e}");
        }

        if is_done.load(Ordering::Acquire) {
            break;
        }
    }
}
