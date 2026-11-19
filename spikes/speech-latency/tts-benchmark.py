"""Times text-to-speech engines on short and long replies (CPU only).

Each run starts a fresh process, so wall time includes process start and
model load. Real-time factor (RTF) = wall seconds / audio seconds; below 1.0
means faster than real time.

Usage: python tts-benchmark.py --llama-dir DIR --lfm-model M.gguf --lfm-mmproj P.gguf \
         --piper-exe piper.exe --piper-voice v.onnx [--piper-voice v2.onnx ...] --out results.json
"""
import argparse, json, os, subprocess, tempfile, time, wave

TEXTS = {
    "short": "Click the Save button in the top left corner.",
    "long": ("You can find the Save button in the top left corner of the window. "
             "Click it once, then choose a folder, give your file a name, and press Enter to finish."),
}


def audio_seconds(path):
    with wave.open(path) as w:
        return w.getnframes() / w.getframerate()


def timed(cmd, stdin=None):
    start = time.perf_counter()
    subprocess.run(cmd, input=stdin, capture_output=True, check=True)
    return time.perf_counter() - start


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--llama-dir", required=True)
    ap.add_argument("--lfm-model", required=True)
    ap.add_argument("--lfm-mmproj", required=True)
    ap.add_argument("--piper-exe", required=True)
    ap.add_argument("--piper-voice", action="append", required=True)
    ap.add_argument("--threads", type=int, default=6)
    ap.add_argument("--repeats", type=int, default=3)
    ap.add_argument("--out")
    a = ap.parse_args()
    a.piper_exe = os.path.abspath(a.piper_exe)
    a.piper_voice = [os.path.abspath(v) for v in a.piper_voice]
    a.llama_dir = os.path.abspath(a.llama_dir)
    a.lfm_model = os.path.abspath(a.lfm_model)
    a.lfm_mmproj = os.path.abspath(a.lfm_mmproj)

    results = []
    with tempfile.TemporaryDirectory() as tmp:
        for kind, text in TEXTS.items():
            for rep in range(a.repeats):
                wav = os.path.join(tmp, "lfm.wav")
                wall = timed([os.path.join(a.llama_dir, "llama-tts.exe"), "-m", a.lfm_model, "-mm", a.lfm_mmproj,
                              "-p", text, "--tts-voice", "female", "-t", str(a.threads), "-o", wav])
                results.append({"engine": "lfm3-audio-2b-q4_k_m", "text": kind, "repeat": rep, "wall_s": round(wall, 2),
                                "audio_s": round(audio_seconds(wav), 2), "rtf": round(wall / audio_seconds(wav), 2)})
                for voice in a.piper_voice:
                    wav = os.path.join(tmp, "piper.wav")
                    wall = timed([a.piper_exe, "-m", voice, "-f", wav, "-q"], stdin=text.encode())
                    results.append({"engine": "piper-" + os.path.basename(voice).replace(".onnx", ""), "text": kind,
                                    "repeat": rep, "wall_s": round(wall, 2), "audio_s": round(audio_seconds(wav), 2),
                                    "rtf": round(wall / audio_seconds(wav), 2)})
    summary = {}
    for r in results:
        summary.setdefault((r["engine"], r["text"]), []).append(r)
    print(f"{'engine':34} {'text':6} {'wall_s':>7} {'audio_s':>8} {'rtf(min/med)':>14}")
    for (engine, text), rows in summary.items():
        rtfs = sorted(r["rtf"] for r in rows)
        print(f"{engine:34} {text:6} {sum(r['wall_s'] for r in rows)/len(rows):7.2f} {rows[0]['audio_s']:8.2f} {rtfs[0]:6.2f}/{rtfs[len(rtfs)//2]:.2f}")
    if a.out:
        with open(a.out, "w") as f:
            json.dump(results, f, indent=2)


if __name__ == "__main__":
    main()
