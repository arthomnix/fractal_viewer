#!/usr/bin/env sh
rm -r pkg
mkdir -p pkg/webgpu
cp index.html pkg
cp index.html pkg/webgpu
wasm-pack build --target web --no-typescript --no-pack . --features webgl
wasm-pack build --target web --no-typescript --no-pack -d pkg/webgpu
cd pkg
zip -r fractal_viewer_web.zip *
cd ..