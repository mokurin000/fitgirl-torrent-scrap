use std::error::Error;

use db_helper::{Record, TABLE, list_games, read_transac};
use polars::{frame::DataFrame, prelude::*};
use polars_excel_writer::PolarsExcelWriter;

fn main() -> Result<(), Box<dyn Error>> {
    let mut titles = vec![];
    let mut torrents = vec![];
    let read = read_transac()?;
    let table = read.open_table(TABLE)?;

    for Record { title, torrent } in list_games(&table)? {
        titles.push(title);
        torrents.push(torrent);
    }

    let title = Series::new("title".into(), titles);
    let torrent = Series::new("torrent".into(), torrents);

    let columns = vec![
        Column::new("title".into(), title),
        Column::new("torrent".into(), torrent),
    ];
    let df = DataFrame::new(columns)?;

    let mut writer = PolarsExcelWriter::new();
    writer.write_dataframe(&df)?;
    writer.save("games.xlsx")?;

    Ok(())
}
