// Scores speech recognition through a local llama-server (OpenAI-compatible audio input).
// Usage: node asr-benchmark.mjs <samples-dir> [serverUrl] [--out results.json]
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const args = process.argv.slice(2);
const outIdx = args.indexOf('--out');
const outPath = outIdx >= 0 ? args.splice(outIdx, 2)[1] : null;
const [dir, server = 'http://127.0.0.1:8090'] = args;
if (!dir) { console.error('usage: node asr-benchmark.mjs <samples-dir> [serverUrl] [--out file]'); process.exit(2); }

const samples = JSON.parse(readFileSync(join(dir, 'samples.json'), 'utf8').replace(/^\uFEFF/, ''));
const normalise = (s) => s.toLowerCase().replace(/[^a-z0-9' ]+/g, ' ').split(/\s+/).filter(Boolean);

function wordErrors(ref, hyp) {
  const d = Array.from({ length: ref.length + 1 }, (_, i) => [i, ...Array(hyp.length).fill(0)]);
  for (let j = 0; j <= hyp.length; j++) d[0][j] = j;
  for (let i = 1; i <= ref.length; i++)
    for (let j = 1; j <= hyp.length; j++)
      d[i][j] = Math.min(d[i - 1][j] + 1, d[i][j - 1] + 1, d[i - 1][j - 1] + (ref[i - 1] === hyp[j - 1] ? 0 : 1));
  return d[ref.length][hyp.length];
}

const rows = [];
let errs = 0, words = 0, totalMs = 0, totalAudioS = 0;
for (const s of samples) {
  const wav = readFileSync(join(dir, s.file));
  const audioS = (wav.length - 44) / 32000; // 16 kHz mono 16-bit
  const started = performance.now();
  const res = await fetch(`${server}/v1/chat/completions`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      temperature: 0,
      max_tokens: 80,
      messages: [
        { role: 'system', content: 'Perform ASR in English.' },
        { role: 'user', content: [{ type: 'input_audio', input_audio: { data: wav.toString('base64'), format: 'wav' } }] },
      ],
    }),
  });
  const ms = performance.now() - started;
  const body = await res.json();
  const hyp = body.choices?.[0]?.message?.content ?? JSON.stringify(body).slice(0, 200);
  const e = wordErrors(normalise(s.text), normalise(hyp));
  errs += e; words += normalise(s.text).length; totalMs += ms; totalAudioS += audioS;
  rows.push({ file: s.file, reference: s.text, hypothesis: hyp.trim(), word_errors: e, audio_s: +audioS.toFixed(2), ms: Math.round(ms), rtf: +(ms / 1000 / audioS).toFixed(2) });
  console.log(`${s.file} audio=${audioS.toFixed(1)}s took=${(ms / 1000).toFixed(1)}s errors=${e}  "${hyp.trim()}"`);
}
const summary = { wer: +(errs / words).toFixed(3), mean_latency_s: +(totalMs / samples.length / 1000).toFixed(2), real_time_factor: +(totalMs / 1000 / totalAudioS).toFixed(2) };
console.log(`\nWER ${(summary.wer * 100).toFixed(1)}%  mean latency ${summary.mean_latency_s}s  real-time factor ${summary.real_time_factor}`);
if (outPath) writeFileSync(outPath, JSON.stringify({ summary, rows }, null, 2));
