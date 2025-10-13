use kanal::{AsyncReceiver, AsyncSender};
use nyquest::{AsyncClient, r#async::Request};

use scraper::{Html, Selector};
use spdlog::info;
use tokio::task;

pub async fn fetch_worker(
    page_rx: AsyncReceiver<u16>,
    client: AsyncClient,
    tx_html: AsyncSender<Html>,
) {
    while let Ok(page) = page_rx.recv().await {
        let url = format!("/page/{page}/");
        let Ok(resp) = client.request(Request::get(url)).await else {
            continue;
        };
        let Ok(text) = resp.text().await else {
            continue;
        };

        let (html, should_end) = task::spawn_blocking(move || {
            let html = scraper::Html::parse_document(&text);
            let should_end = html
                .select(&Selector::parse("h1.page-title").unwrap())
                .next()
                .is_some();
            (html, should_end)
        })
        .await
        .unwrap();

        let _ = tx_html.send(html).await;
        if should_end {
            _ = page_rx.close();
            return;
        }
        info!("processed page {page}");
    }
}
