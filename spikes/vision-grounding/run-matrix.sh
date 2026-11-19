#!/usr/bin/env bash
# Runs the grounding benchmark for several model/image/prompt combinations.
# usage: run-matrix.sh <llama-dir> <models-dir> <data-dir> <out-dir>
# <data-dir> must hold synthetic-ui.png, synthetic-640.png and synthetic-ui.json.
set -uo pipefail
llama="$1"; models="$2"; data="$3"; out="$4"; port=8089
mkdir -p "$out"
here="$(cd "$(dirname "$0")" && pwd)"

run_model() { # name model-file mmproj-file
  name="$1"; model="$2"; mmproj="$3"
  taskkill //F //IM llama-server.exe >/dev/null 2>&1; sleep 2
  "$llama/llama-server.exe" -m "$models/$model" --mmproj "$models/$mmproj" -t 6 -c 4096 --port $port >"$out/$name.server.log" 2>&1 &
  for _ in $(seq 1 60); do sleep 2; curl -s localhost:$port/health | grep -q ok && break; done
  for img in synthetic-640 synthetic-ui; do
    for variant in bbox pixel; do
      echo "== $name $img $variant"
      node "$here/benchmark.mjs" "$data/$img.png" "$data/synthetic-ui.json" http://127.0.0.1:$port "$variant" --out "$out/$name.$img.$variant.json" | tail -1
    done
  done
  taskkill //F //IM llama-server.exe >/dev/null 2>&1
}

run_model lfm25-vl-450m-q4km LFM2.5-VL-450M-Q4_K_M.gguf mmproj-LFM2.5-VL-450m-Q8_0.gguf
run_model lfm25-vl-1.6b-q4km LFM2.5-VL-1.6B-Q4_K_M.gguf mmproj-LFM2.5-VL-1.6b-Q8_0.gguf
run_model lfm3-vl-2b-q4km    LFM3-VL-2B-Q4_K_M.gguf     mmproj-LFM3-VL-2B-Q8_0.gguf
