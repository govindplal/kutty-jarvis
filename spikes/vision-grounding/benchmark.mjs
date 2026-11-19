// Asks a local llama-server vision model to locate each labelled button and
// scores the answer against ground truth.
// Usage: node benchmark.mjs <image.png> <truth.json> [serverUrl] [variant] [--out results.json]
import { readFileSync, writeFileSync } from 'node:fs';

const args = process.argv.slice(2);
const outIdx = args.indexOf('--out');
const outPath = outIdx >= 0 ? args.splice(outIdx, 2)[1] : null;
const [imagePath, truthPath, server = 'http://127.0.0.1:8089', variant = 'pixel'] = args;
if (!imagePath || !truthPath) {
  console.error('usage: node benchmark.mjs <image.png> <truth.json> [serverUrl] [pixel|normalized|bbox]');
  process.exit(2);
}

const truth = JSON.parse(readFileSync(truthPath, 'utf8').replace(/^﻿/, ''));
const imageUrl = `data:image/png;base64,${readFileSync(imagePath).toString('base64')}`;

const prompts = {
  pixel: (l) => `The image is ${truth.width}x${truth.height} pixels. Where is the "${l}" button? Reply with only its center as [POINT:x,y] in pixel coordinates.`,
  normalized: (l) => `Where is the "${l}" button? Reply with only its center as [POINT:x,y] where x and y are fractions between 0 and 1 of the image width and height.`,
  bbox: (l) => `Locate the "${l}" button. Reply with only its bounding box as [x1,y1,x2,y2] with each value a fraction between 0 and 1 of the image size.`,
};

function parse(text) {
  const nums = (text.match(/-?\d*\.?\d+/g) ?? []).map(Number);
  if (variant === 'bbox' && nums.length >= 4) {
    return { x: ((nums[0] + nums[2]) / 2) * truth.width, y: ((nums[1] + nums[3]) / 2) * truth.height };
  }
  if (nums.length < 2) return null;
  return variant === 'pixel' ? { x: nums[0], y: nums[1] } : { x: nums[0] * truth.width, y: nums[1] * truth.height };
}

let hits = 0, totalErr = 0, totalMs = 0;
const rows = [];
for (const t of truth.targets) {
  const started = performance.now();
  const res = await fetch(`${server}/v1/chat/completions`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      temperature: 0,
      max_tokens: 40,
      messages: [{ role: 'user', content: [{ type: 'image_url', image_url: { url: imageUrl } }, { type: 'text', text: prompts[variant](t.label) }] }],
    }),
  });
  const ms = performance.now() - started;
  const reply = (await res.json()).choices?.[0]?.message?.content ?? '';
  const p = parse(reply);
  const err = p ? Math.hypot(p.x - t.x, p.y - t.y) : Infinity;
  const hit = p && Math.abs(p.x - t.x) <= t.w / 2 && Math.abs(p.y - t.y) <= t.h / 2;
  if (hit) hits += 1;
  if (Number.isFinite(err)) totalErr += err;
  totalMs += ms;
  rows.push({ label: t.label, hit: Boolean(hit), error_px: Number.isFinite(err) ? Math.round(err) : null, ms: Math.round(ms), reply: reply.trim() });
  console.log(`${t.label.padEnd(9)} hit=${hit ? 'Y' : 'N'} err=${Number.isFinite(err) ? err.toFixed(0) : 'n/a'}px ${ms.toFixed(0)}ms reply=${JSON.stringify(reply.trim())}`);
}
if (outPath) {
  writeFileSync(outPath, JSON.stringify({
    variant,
    image: imagePath,
    hits,
    targets: truth.targets.length,
    mean_error_px: Math.round(totalErr / truth.targets.length),
    mean_latency_s: +(totalMs / truth.targets.length / 1000).toFixed(1),
    results: rows,
  }, null, 2));
}
console.log(`\n[${variant}] hit rate ${hits}/${truth.targets.length}, mean error ${(totalErr / truth.targets.length).toFixed(0)}px, mean latency ${(totalMs / truth.targets.length / 1000).toFixed(1)}s`);
