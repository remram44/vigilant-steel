#!/bin/sh

cd "$(dirname "$0")/.."
docker run -t --rm -v "$PWD:/src" -v "$HOME/.cargo/registry:/root/.cargo/registry" -e EMMAKEN_CFLAGS='-s USE_SDL=2 --preload-file assets' remram/emscripten-rust-sdl sh -c 'cd client-piston && cargo build --release --target=asmjs-unknown-emscripten --no-default-features'
cp target/asmjs-unknown-emscripten/release/deps/client-*.data .
