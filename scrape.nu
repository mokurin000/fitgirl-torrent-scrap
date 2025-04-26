$env.RUST_LOG = "fitgirl_torrent_scrap=DEBUG"
^cargo r -r --bin fitgirl-torrent-scrap -- --start-page 1 --filter adult-only --save-dir output/adult
^cargo r -r --bin fitgirl-torrent-scrap -- --start-page 1 --filter no-adult --save-dir output/other