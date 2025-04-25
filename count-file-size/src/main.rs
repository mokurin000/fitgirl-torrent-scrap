use std::{fs, path::Path};

use humansize::BINARY;
use librqbit_buffers::ByteBuf;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

fn main() -> anyhow::Result<()> {
    let torrents = fs::read_dir("./output")?;

    let mut torrents_vec = Vec::new();
    for entry in torrents {
        let entry = entry?;
        if !Path::new(&entry.file_name())
            .extension()
            .is_some_and(|ext| ext.to_str() == Some("torrent"))
        {
            continue;
        }

        let data = fs::read(entry.path())?;
        torrents_vec.push(data);
    }

    let total_length = torrents_vec
        .par_iter()
        .filter_map(|bytes| {
            librqbit_core::torrent_metainfo::torrent_from_bytes::<ByteBuf>(&bytes).ok()
        })
        .filter_map(|meta| {
            meta.info
                .iter_file_lengths()
                .ok()
                .map(|length| length.sum::<u64>())
        })
        .sum::<u64>();

    let humansize = humansize::format_size(total_length, BINARY);
    println!("{humansize}");

    Ok(())
}
