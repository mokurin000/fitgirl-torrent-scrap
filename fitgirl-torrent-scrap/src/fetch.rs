use kanal::{AsyncReceiver, AsyncSender};
use nyquest::{AsyncClient, r#async::Request};

use tracing::info;

pub async fn fetch_worker(
    page_rx: AsyncReceiver<u16>,
    client: AsyncClient,
    tx_html: AsyncSender<String>,
) {
    while let Ok(page) = page_rx.recv().await {
        let url = format!("/page/{page}/");
        let Ok(resp) = client.request(Request::get(url)).await else {
            continue;
        };
        let Ok(text) = resp.text().await else {
            continue;
        };

        let _ = tx_html.send(text).await;
        info!("processed page {page}");
    }
}
