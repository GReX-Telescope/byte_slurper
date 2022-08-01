#!/usr/bin/env bash

PKG_CONFIG_PATH=~/pkg_config CC=gcc LIBCLANG_PATH=~/.guix_profile/lib cargo build --release
