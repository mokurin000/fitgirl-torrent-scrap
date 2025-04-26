pub mod decrypt_torrents;
pub mod extract_links;
pub mod fetch;

pub const FETCH_WORKERS: usize = 5;
pub const DECRYPT_WORKERS: usize = 10;

#[derive(Clone, Copy, strum::EnumString)]
#[strum(serialize_all = "kebab-case")]
pub enum FilterType {
    AdultOnly,
    NoAdult,
    None,
}
