# tigo-exporter
Uses daqs csv data on the tigo CCA and export it to prometheus.

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

Per default the metrics can be fetched on `http://TIGO_IP:9980/metrics`
