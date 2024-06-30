#!/bin/bash
cd "$(dirname "$0")"
rm -rf pkg
mkdir pkg
cp index.html main.js style.css pkg
wasm-pack build --target web --no-typescript --no-pack -d web/pkg
zip -r fractal_viewer_web.zip pkg/*