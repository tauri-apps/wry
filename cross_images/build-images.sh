#!/bin/bash
docker build --tag phnz/tauri:aarch64-unknown-linux-gnu ./aarch64-unknown-linux-gnu
docker build --tag phnz/tauri:arm-unknown-linux-gnueabihf ./arm-unknown-linux-gnueabihf
docker build --tag phnz/tauri:armv7-unknown-linux-gnueabihf ./armv7-unknown-linux-gnueabihf