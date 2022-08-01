#!/usr/bin/env bash

export PKG_CONFIG_PATH=~/pkg_config
export CC=gcc
export LIBCLANG_PATH=~/.guix-profile/lib
cargo build --release
