import asyncio
import json
import os
import sys
import edge_tts

async def main():
    if len(sys.argv) < 3 or sys.argv[1] != "--spec":
        raise SystemExit("Usage: tts_edge.py --spec <spec.json>")
    with open(sys.argv[2], "r", encoding="utf-8") as f:
        spec = json.load(f)
    text = (spec.get("text") or "").strip()
    voice = (spec.get("voice") or "es-AR-TomasNeural").strip()
    rate = (spec.get("rate") or "+2%").strip()
    pitch = (spec.get("pitch") or "+0Hz").strip()
    out_path = spec["out"]
    words_out = spec.get("words_out")
    if not text:
        raise SystemExit("Empty narration text")

    words = []
    audio_bytes = bytearray()

    try:
        communicate = edge_tts.Communicate(
            text, voice, rate=rate, pitch=pitch, boundary="WordBoundary"
        )
        async for chunk in communicate.stream():
            ctype = chunk.get("type")
            if ctype == "audio":
                data = chunk.get("data") or b""
                audio_bytes.extend(data)
            elif ctype == "WordBoundary":
                w_text = (chunk.get("text") or "").strip()
                off = float(chunk.get("offset") or 0) / 10_000_000.0
                dur = float(chunk.get("duration") or 0) / 10_000_000.0
                if w_text:
                    words.append({
                        "start": round(off, 3),
                        "end": round(max(off + 0.12, off + dur), 3),
                        "text": w_text,
                    })
    except Exception:
        audio_bytes.clear()
        words.clear()

    if not audio_bytes:
        communicate = edge_tts.Communicate(text, voice, rate=rate, pitch=pitch)
        await communicate.save(out_path)
    else:
        with open(out_path, "wb") as out_f:
            out_f.write(bytes(audio_bytes))

    if words_out:
        with open(words_out, "w", encoding="utf-8") as wf:
            json.dump(words, wf, ensure_ascii=False)

if __name__ == "__main__":
    asyncio.run(main())
