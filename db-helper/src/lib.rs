use std::sync::LazyLock;

use redb::{
    Database, ReadTransaction, ReadableDatabase as _, ReadableTable, TableDefinition,
    WriteTransaction,
};

pub fn read_transac() -> Result<ReadTransaction, redb::Error> {
    Ok(DATABASE.begin_read()?)
}

pub fn write_transac() -> Result<WriteTransaction, redb::Error> {
    Ok(DATABASE.begin_write()?)
}

pub fn query_torrent(
    tsx: &ReadTransaction,
    title: impl Into<String>,
) -> Result<Option<String>, redb::Error> {
    let result = tsx.open_table(GAME_TORRENT)?.get(title.into())?;
    Ok(result.map(|g| g.value()))
}

pub fn query_date(
    tsx: &ReadTransaction,
    title: impl Into<String>,
) -> Result<Option<String>, redb::Error> {
    let result = tsx.open_table(GAME_PUBLISH_DATE)?.get(title.into())?;
    Ok(result.map(|g| g.value()))
}

pub fn list_games(
    table: &impl ReadableTable<String, String>,
) -> Result<impl Iterator<Item = Record>, redb::Error> {
    Ok(table
        .iter()?
        .filter_map(Result::ok)
        .map(|(title, value)| (title.value(), value.value()))
        .map(|(title, value)| Record { title, value }))
}

pub fn add_game(
    tsx: &WriteTransaction,
    title: impl Into<String>,
    torrent_name: impl Into<String>,
) -> Result<(), redb::Error> {
    let mut table = tsx.open_table(GAME_TORRENT)?;
    table.insert(title.into(), torrent_name.into())?;
    Ok(())
}

pub fn add_game_date(
    tsx: &WriteTransaction,
    title: impl Into<String>,
    torrent_name: impl Into<String>,
) -> Result<(), redb::Error> {
    let mut table = tsx.open_table(GAME_PUBLISH_DATE)?;
    table.insert(title.into(), torrent_name.into())?;
    Ok(())
}

pub const GAME_TORRENT: TableDefinition<String, String> = TableDefinition::new("games");
pub const GAME_PUBLISH_DATE: TableDefinition<String, String> = TableDefinition::new("games_date");
static DATABASE: LazyLock<Database> = LazyLock::new(|| {
    let mut db = Database::create("game.redb").expect("failed to open database!");
    _ = db.compact();

    // create empty table if not existing
    let tsx = db.begin_write().unwrap();
    tsx.open_table(GAME_TORRENT).unwrap();
    tsx.open_table(GAME_PUBLISH_DATE).unwrap();
    tsx.commit().expect("failed to init table!");
    db
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub title: String,
    pub value: String,
}
