// Transcribes one WAV with LFM3-Audio through llama-server and scores it.
// Usage: node transcribe-wav.mjs <file.wav> [--ref "reference text"] [--server http://127.0.0.1:8090]
// Default WAV: %TEMP%/kutty-jarvis/last-recording.wav (written by debug builds of the app).
// If nothing answers at the server URL, the script starts llama-server itself and stops it at the end.
import { readFileSync, existsSync } from 'node:fs';
import { spawn } from 'node:child_process';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const MAIN = process.env.KJ_MAIN ?? 'C:/Users/govin/repos/portfolios/jarvis';
const MODEL = `${MAIN}/.models/LFM3-Audio-2B-Q4_K_M.gguf`;
const MMPROJ = `${MAIN}/.models/mmproj-LFM3-Audio-2B-Q8_0.gguf`;
const SERVER_EXE = `${MAIN}/.tools/llama/llama-server.exe`;

const args = process.argv.slice(2);
const take = (flag) => { const i = args.indexOf(flag); return i >= 0 ? args.splice(i, 2)[1] : null; };
const ref = take('--ref');
const server = take('--server') ?? 'http://127.0.0.1:8090';
const wavPath = args[0] ?? join(tmpdir(), 'kutty-jarvis', 'last-recording.wav');
if (!existsSync(wavPath)) { console.error(`no such file: ${wavPath}`); process.exit(2); }

const normalise = (s) => s.toLowerCase().replace(/[^a-z0-9' ]+/g, ' ').split(/\s+/).filter(Boolean);
function wordErrors(r, h) {
  const d = Array.from({ length: r.length + 1 }, (_, i) => [i, ...Array(h.length).fill(0)]);
  for (let j = 0; j <= h.length; j++) d[0][j] = j;
  for (let i = 1; i <= r.length; i++)
    for (let j = 1; j <= h.length; j++)
      d[i][j] = Math.min(d[i - 1][j] + 1, d[i][j - 1] + 1, d[i - 1][j - 1] + (r[i - 1] === h[j - 1] ? 0 : 1));
  return d[r.length][h.length];
}

const alive = async () => { try { return (await fetch(`${server}/health`)).ok; } catch { return false; } };

let child = null;
if (!(await alive())) {
  const port = new URL(server).port || '8090';
  child = spawn(SERVER_EXE, ['-m', MODEL, '--mmproj', MMPROJ, '-t', '6', '-c', '4096', '--port', port], { stdio: 'ignore' });
  console.error(`started llama-server (pid ${child.pid}), waiting for it to load...`);
  for (let i = 0; i < 120 && !(await alive()); i++) await new Promise((r) => setTimeout(r, 1000));
  if (!(await alive())) { child.kill(); console.error('llama-server did not become ready'); process.exit(1); }
}

try {
  const wav = readFileSync(wavPath);
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
  const hyp = (body.choices?.[0]?.message?.content ?? JSON.stringify(body).slice(0, 200)).trim();
  console.log(`file:       ${wavPath}`);
  console.log(`audio:      ${audioS.toFixed(2)} s`);
  console.log(`transcript: ${hyp}`);
  console.log(`latency:    ${(ms / 1000).toFixed(2)} s (real-time factor ${(ms / 1000 / audioS).toFixed(2)})`);
  if (ref) {
    const r = normalise(ref);
    const e = wordErrors(r, normalise(hyp));
    console.log(`reference:  ${ref}`);
    console.log(`WER:        ${(100 * e / r.length).toFixed(1)}% (${e} errors in ${r.length} words)`);
  }
} finally {
  if (child) child.kill(); // only the process this script started
}
