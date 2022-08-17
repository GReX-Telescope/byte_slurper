#!/usr/bin/env bash

export CC=gcc
export LIBCLANG_PATH=~/.guix-profile/lib
cargo build --release
