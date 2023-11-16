# tigo-exporter
Uses the direct CCA access /cgi-bin/summary_data url to retrieve slightly delayed Tigo data
and export to prometheus.

## Build

Build it like any other rust project:

    cd tigo-exporter
    cargo build

To cross compile for tigo CCA, install `cross`

    cargo install cross

And run either `make` or call cross directly:

    cross build --target=armv7-unknown-linux-gnueabihf --release

## Install

Copy the build binary into `/mnt/ffs/bin` on your tigo CCA and
there is also a start script `S060_tigo_exporter` that should be placed in `/mnt/ffs/etc/rc.d`
