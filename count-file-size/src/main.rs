use std::{
    fs::{self, read_dir},
    path::{Path, PathBuf},
};

use humansize::BINARY;
use librqbit_buffers::ByteBuf;
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};

#[derive(argh::FromArgs)]
#[argh(description = "count file size from torrent metainfo")]
struct Args {
    #[argh(positional)]
    torrent_paths: Vec<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let Args { torrent_paths } = argh::from_env();
    let mut result = vec![];
    for torrent_path in torrent_paths {
        visit_dirs_or_file(torrent_path, &mut result)?;
    }

    let torrents_vec: Vec<_> = result
        .into_par_iter()
        .filter_map(|path| fs::read(path).ok())
        .collect();

    let total_length = torrents_vec
        .par_iter()
        .filter_map(|bytes| {
            librqbit_core::torrent_metainfo::torrent_from_bytes::<ByteBuf>(bytes).ok()
        })
        .filter_map(|meta| {
            meta.info
                .iter_file_lengths()
                .ok()
                .map(|length| length.sum::<u64>())
        })
        .sum::<u64>();

    let humansize = humansize::format_size(total_length, BINARY);
    println!("Total: {humansize}");

    Ok(())
}

fn visit_dirs_or_file(path: impl AsRef<Path>, append_to: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    let path = path.as_ref();
    if path.is_file() {
        append_to.push(path.to_path_buf());
        return Ok(());
    }

    let dir = read_dir(path)?;
    for entry in dir.flatten() {
        let path = entry.path();

        if path.is_dir() {
            visit_dirs_or_file(path, append_to)?;
        } else {
            if path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext == "torrent")
            {
                append_to.push(path);
            }
        }
    }

    Ok(())
}
