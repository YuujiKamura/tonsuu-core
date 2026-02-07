#!/bin/bash
# Build WASM package and include prompt-spec.json
set -e
wasm-pack build --target web --features wasm
cp prompt-spec.json pkg/prompt-spec.json
echo "prompt-spec.json copied to pkg/"
