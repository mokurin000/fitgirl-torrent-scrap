use std::sync::LazyLock;

use redb::{Database, ReadTransaction, ReadableTable, TableDefinition, WriteTransaction};

pub fn read_transac() -> Result<ReadTransaction, redb::Error> {
    Ok(DATABASE.begin_read()?)
}

pub fn write_transac() -> Result<WriteTransaction, redb::Error> {
    Ok(DATABASE.begin_write()?)
}

pub fn query_game(
    tsx: &ReadTransaction,
    title: impl Into<String>,
) -> Result<Option<String>, redb::Error> {
    let result = tsx.open_table(TABLE)?.get(title.into())?;
    Ok(result.map(|g| g.value()))
}

pub fn list_games(
    table: &impl ReadableTable<String, String>,
) -> Result<impl Iterator<Item = Record>, redb::Error> {
    Ok(table
        .iter()?
        .filter_map(Result::ok)
        .map(|(title, torrent)| (title.value(), torrent.value()))
        .map(|(title, torrent)| Record { title, torrent }))
}

pub fn add_game(
    tsx: &WriteTransaction,
    title: impl Into<String>,
    torrent_name: impl Into<String>,
) -> Result<(), redb::Error> {
    let mut table = tsx.open_table(TABLE)?;
    table.insert(title.into(), torrent_name.into())?;
    Ok(())
}

pub const TABLE: TableDefinition<String, String> = TableDefinition::new("games");
static DATABASE: LazyLock<Database> = LazyLock::new(|| {
    let mut db = Database::create("game.redb").expect("failed to open database!");
    _ = db.upgrade(); // try to update DB

    // create empty table if not existing
    let tsx = db.begin_write().unwrap();
    tsx.open_table(TABLE).unwrap();
    tsx.commit().expect("failed to init table!");
    db
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub title: String,
    pub torrent: String,
}
