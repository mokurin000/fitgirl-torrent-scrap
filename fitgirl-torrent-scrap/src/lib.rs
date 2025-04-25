pub mod decrypt;
pub mod extract;
pub mod fetch;

pub const FETCH_WORKERS: usize = 5;
pub const DECRYPT_WORKERS: usize = 10;
pub const OUTPUT_DIR: &str = "./output/";
