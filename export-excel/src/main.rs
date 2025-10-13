use std::error::Error;

use db_helper::{GAME_TORRENT, Record, list_games, query_date, read_transac};
use polars::{frame::DataFrame, prelude::*};
use polars_excel_writer::PolarsExcelWriter;

fn main() -> Result<(), Box<dyn Error>> {
    let mut titles = vec![];
    let mut torrents = vec![];
    let mut publish_dates = vec![];
    let read = read_transac()?;
    let torrent = read.open_table(GAME_TORRENT)?;

    for Record { title, value } in list_games(&torrent)? {
        if let Some(date) = query_date(&read, &title)? {
            publish_dates.push(date);
        } else {
            publish_dates.push(String::new());
        }

        titles.push(title);
        torrents.push(value);
    }

    let title = Series::new("title".into(), titles);
    let torrent = Series::new("torrent".into(), torrents);
    let publish_date = Series::new("publish_date".into(), publish_dates);

    let columns = vec![
        Column::new("title".into(), title),
        Column::new("torrent".into(), torrent),
        Column::new("publish_date".into(), publish_date),
    ];
    let df = DataFrame::new(columns)?;
    let mut df = df.unique_stable(Some(&["torrent".into()]), UniqueKeepStrategy::Last, None)?;
    df.sort_in_place(
        ["publish_date"],
        SortMultipleOptions::new().with_order_descending(true),
    )?;

    let mut writer = PolarsExcelWriter::new();
    writer.set_autofit(true);
    writer.write_dataframe(&df)?;
    writer.save("games.xlsx")?;

    Ok(())
}
