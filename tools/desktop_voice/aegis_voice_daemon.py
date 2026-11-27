#!/usr/bin/env python3
"""
Aegis OS — PC Always-On Background Voice Companion (EPIC 63 / CORE-414)

Keeps Aegis listening continuously even when the web browser is completely closed:
  - 0 LLM tokens spent during room silence (local breath-aware RMS VAD with 500ms pre-roll)
  - Multi-speaker voiceprint verification via /api/siren/utterance
  - Dynamic wake-word ("Aegis escuchame" or "<CustomAssistantName> escuchame")
  - 8-second follow-up conversation window after each spoken reply
  - Global Windows hotkey (Alt+Shift+A / Ctrl+Shift+Space) for instant push-to-talk
  - Optional Windows Startup installation (--install-startup / --uninstall-startup)
"""

from __future__ import annotations

import argparse
import base64
from collections import deque
import ctypes
from dataclasses import dataclass
import json
import math
import os
import queue
import struct
import subprocess
import sys
import threading
import time
from typing import Deque, List, Optional, Tuple
import urllib.error
import urllib.parse
import urllib.request

SAMPLE_RATE = 16000
FRAME_SAMPLES = 480  # 30ms @ 16kHz


@dataclass
class VadConfig:
    sample_rate: int = SAMPLE_RATE
    pre_roll_ms: int = 500
    wake_hangover_ms: int = 1400
    command_hangover_ms: int = 2400
    initial_wait_ms: int = 10000
    max_utterance_ms: int = 40000
    min_utterance_ms: int = 450
    min_rms_floor: float = 0.012


def compute_rms_pcm16(pcm_bytes: bytes) -> float:
    """Computes normalized [0..1] RMS of 16-bit signed LE PCM bytes."""
    count = len(pcm_bytes) // 2
    if count == 0:
        return 0.0
    samples = struct.unpack(f"<{count}h", pcm_bytes[: count * 2])
    sq = sum((s / 32768.0) ** 2 for s in samples)
    return math.sqrt(sq / count)


class BreathAwareVadSegmenter:
    """
    Segments spoken utterances without cutting off mid-sentence breathing pauses.
    Includes a circular pre-roll buffer so initial consonants are always preserved.
    """

    def __init__(self, config: Optional[VadConfig] = None) -> None:
        self.cfg = config or VadConfig()
        frame_ms = max(1, round((FRAME_SAMPLES / self.cfg.sample_rate) * 1000))
        max_pre_frames = max(1, round(self.cfg.pre_roll_ms / frame_ms))
        self.pre_roll: Deque[bytes] = deque(maxlen=max_pre_frames)
        self.noise_ema: float = 0.006
        self.in_speech: bool = False
        self.consecutive_speech: int = 0
        self.speech_chunks: List[bytes] = []
        self.speech_start_ms: int = 0
        self.last_speech_ms: int = 0
        self.mode: str = "wakeword"  # "wakeword" | "direct" | "followup"
        self.direct_started_ms: int = 0

    def arm_direct(self, mode: str = "direct", now_ms: Optional[int] = None) -> None:
        now = now_ms if now_ms is not None else int(time.monotonic() * 1000)
        self.reset_utterance()
        self.mode = mode
        self.direct_started_ms = now

    def reset_utterance(self) -> None:
        self.in_speech = False
        self.consecutive_speech = 0
        self.speech_chunks = []

    def feed_frame(
        self, pcm_frame: bytes, now_ms: Optional[int] = None
    ) -> Optional[Tuple[bytes, str]]:
        """
        Feed a single PCM16 frame. Returns `(pcm_bytes, mode)` when a complete
        utterance finishes, or `None` while still buffering/waiting.
        """
        now = now_ms if now_ms is not None else int(time.monotonic() * 1000)
        rms = compute_rms_pcm16(pcm_frame)
        threshold = max(self.cfg.min_rms_floor, self.noise_ema * 2.4)

        if not self.in_speech:
            self.pre_roll.append(pcm_frame)
            if rms > threshold:
                self.consecutive_speech += 1
                if self.consecutive_speech >= 2:
                    self.in_speech = True
                    self.speech_start_ms = now
                    self.last_speech_ms = now
                    self.speech_chunks = list(self.pre_roll)
            else:
                self.consecutive_speech = 0
                self.noise_ema = self.noise_ema * 0.95 + rms * 0.05
                if (
                    self.mode == "direct"
                    and now - self.direct_started_ms >= self.cfg.initial_wait_ms
                ):
                    self.mode = "wakeword"
            return None

        self.speech_chunks.append(pcm_frame)
        if rms > threshold * 0.85:
            self.last_speech_ms = now

        hangover = (
            self.cfg.wake_hangover_ms
            if self.mode == "wakeword"
            else self.cfg.command_hangover_ms
        )
        elapsed_silence = now - self.last_speech_ms
        total_elapsed = now - self.speech_start_ms

        if elapsed_silence >= hangover or total_elapsed >= self.cfg.max_utterance_ms:
            payload = b"".join(self.speech_chunks)
            captured_mode = self.mode
            self.reset_utterance()
            if captured_mode == "direct":
                self.mode = "wakeword"

            duration_ms = (len(payload) // 2) * 1000 // self.cfg.sample_rate
            if duration_ms >= self.cfg.min_utterance_ms:
                return payload, captured_mode

        return None


def play_tone(kind: str = "ready") -> None:
    if sys.platform.startswith("win"):
        try:
            import winsound

            if kind == "ready":
                winsound.Beep(880, 90)
            elif kind == "rejected":
                winsound.Beep(220, 140)
            else:
                winsound.Beep(660, 75)
        except Exception:
            pass


def speak_text_native(text: str) -> None:
    clean = " ".join(text.replace("*", "").replace("`", "").split())[:900]
    if not clean:
        return
    if sys.platform.startswith("win"):
        ps_script = (
            "[Console]::InputEncoding = [System.Text.Encoding]::UTF8; "
            "Add-Type -AssemblyName System.Speech; "
            "$s = New-Object System.Speech.Synthesis.SpeechSynthesizer; "
            "$s.Rate = 1; "
            "$s.Speak([Console]::In.ReadToEnd());"
        )
        subprocess.run(
            ["powershell", "-NoProfile", "-Command", ps_script],
            input=clean.encode("utf-8"),
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )
        return
    try:
        subprocess.run(["espeak", clean], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False)
    except FileNotFoundError:
        pass


def parse_chat_ws_frame(frame: dict) -> Tuple[Optional[str], bool, Optional[str]]:
    """
    Parses a WebSocket frame from `ank-http` (`/ws/chat/:tenant_id`) into
    `(output_chunk, is_done, error_msg)`. Also keeps fallback support for
    legacy `{type: "token"|"status"|"error"}` frames.
    """
    event = frame.get("event")
    if event == "kernel_event":
        data = frame.get("data")
        if isinstance(data, dict):
            if "output" in data and data["output"] is not None:
                return (str(data["output"]), False, None)
            if "error" in data and data["error"] is not None:
                return (None, True, str(data["error"]))
            status_update = data.get("status_update")
            if isinstance(status_update, dict) and status_update.get("state") in (
                "STATE_COMPLETED",
                "STATE_FAILED",
            ):
                return (None, True, None)
        return (None, False, None)

    if event == "error":
        data = frame.get("data")
        if isinstance(data, dict):
            msg = data.get("message") or data.get("code") or str(data)
        else:
            msg = str(data or "Unknown WebSocket error")
        return (None, True, str(msg))

    kind = frame.get("type")
    if kind == "token":
        return (str(frame.get("data", "")), False, None)
    if kind == "status" and str(frame.get("data", "")) in ("idle", "completed"):
        return (None, True, None)
    if kind == "error":
        return (None, True, str(frame.get("data", "")))
    return (None, False, None)


class AegisHttpBridge:
    def __init__(
        self,
        base_url: str,
        tenant_id: str,
        session_key: str,
        passphrase: Optional[str] = None,
    ) -> None:
        self.base_url = base_url.rstrip("/")
        self.tenant_id = tenant_id
        self.session_key = session_key
        self._passphrase = passphrase

    @classmethod
    def _request_login_token(cls, base_url: str, tenant_id: str, passphrase: str) -> str:
        payload = json.dumps({"tenant_id": tenant_id, "session_key": passphrase}).encode("utf-8")
        req = urllib.request.Request(
            f"{base_url.rstrip('/')}/api/auth/login",
            data=payload,
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        with urllib.request.urlopen(req, timeout=15) as resp:
            data = json.loads(resp.read().decode("utf-8"))
        token = (
            data.get("session_token")
            or data.get("session_key")
            or data.get("token")
            or passphrase
        )
        if not token:
            raise RuntimeError(f"Login failed: {data}")
        return str(token)

    @classmethod
    def login(cls, base_url: str, tenant_id: str, passphrase: str) -> "AegisHttpBridge":
        token = cls._request_login_token(base_url, tenant_id, passphrase)
        return cls(base_url, tenant_id, token, passphrase=passphrase)

    def refresh_session(self) -> bool:
        if not self._passphrase:
            return False
        self.session_key = self._request_login_token(
            self.base_url, self.tenant_id, self._passphrase
        )
        return True

    def _headers(self) -> dict:
        return {
            "Content-Type": "application/json",
            "x-citadel-tenant": self.tenant_id,
            "x-citadel-key": self.session_key,
        }

    def post_utterance(self, pcm_bytes: bytes, mode: str, lang: str = "es-AR") -> dict:
        body = json.dumps(
            {
                "pcm_b64": base64.b64encode(pcm_bytes).decode("ascii"),
                "sample_rate": SAMPLE_RATE,
                "mode": mode,
                "lang": lang,
            }
        ).encode("utf-8")
        for attempt in range(2):
            req = urllib.request.Request(
                f"{self.base_url}/api/siren/utterance",
                data=body,
                headers=self._headers(),
                method="POST",
            )
            try:
                with urllib.request.urlopen(req, timeout=30) as resp:
                    return json.loads(resp.read().decode("utf-8"))
            except urllib.error.HTTPError as err:
                if err.code == 401 and attempt == 0 and self.refresh_session():
                    continue
                raise
        raise RuntimeError("Failed to post utterance after session refresh")

    def send_chat_and_wait(self, prompt: str) -> str:
        """Sends a prompt via Aegis WebSocket chat and waits for the assistant response."""
        import websockets.sync.client as ws_client  # type: ignore

        ws_base = self.base_url.replace("https://", "wss://").replace("http://", "ws://")
        encoded_tenant = urllib.parse.quote(self.tenant_id, safe="")
        url = f"{ws_base}/ws/chat/{encoded_tenant}"
        reply_parts: List[str] = []

        with ws_client.connect(
            url,
            additional_headers={"x-citadel-key": self.session_key},
            open_timeout=10,
        ) as ws:
            ws.send(json.dumps({"action": "submit", "prompt": prompt}))
            while True:
                raw = ws.recv(timeout=90)
                frame = json.loads(raw)
                chunk, is_done, error_msg = parse_chat_ws_frame(frame)
                if chunk is not None:
                    reply_parts.append(chunk)
                if error_msg is not None:
                    return f"Error: {error_msg}"
                if is_done:
                    break
        return "".join(reply_parts).strip()


def install_windows_startup(base_url: str, tenant_id: str) -> None:
    if not sys.platform.startswith("win"):
        raise RuntimeError("--install-startup solo aplica a Windows.")
    startup_dir = os.path.join(
        os.environ.get("APPDATA", ""),
        "Microsoft",
        "Windows",
        "Start Menu",
        "Programs",
        "Startup",
    )
    os.makedirs(startup_dir, exist_ok=True)
    vbs_path = os.path.join(startup_dir, "AegisVoiceDaemon.vbs")
    script_path = os.path.abspath(__file__)
    py_exe = sys.executable.replace("python.exe", "pythonw.exe")
    if not os.path.exists(py_exe):
        py_exe = sys.executable
    cmd = f'"{py_exe}" "{script_path}" --url "{base_url}" --tenant "{tenant_id}"'
    content = (
        'Set WshShell = CreateObject("WScript.Shell")\n'
        f'WshShell.Run "{cmd.replace(chr(34), chr(34) * 2)}", 0, False\n'
    )
    with open(vbs_path, "w", encoding="utf-8") as f:
        f.write(content)
    print(f"Instalado en inicio automático de Windows: {vbs_path}")


def uninstall_windows_startup() -> None:
    startup_dir = os.path.join(
        os.environ.get("APPDATA", ""),
        "Microsoft",
        "Windows",
        "Start Menu",
        "Programs",
        "Startup",
    )
    vbs_path = os.path.join(startup_dir, "AegisVoiceDaemon.vbs")
    if os.path.exists(vbs_path):
        os.remove(vbs_path)
        print(f"Eliminado de inicio automático: {vbs_path}")


def start_hotkey_listener(on_trigger) -> None:
    """Registers Alt+Shift+A globally on Windows without requiring elevated rights."""
    if not sys.platform.startswith("win"):
        return

    def _loop() -> None:
        user32 = ctypes.windll.user32
        MOD_ALT = 0x0001
        MOD_SHIFT = 0x0004
        VK_A = 0x41
        if not user32.RegisterHotKey(None, 1, MOD_ALT | MOD_SHIFT, VK_A):
            print("[AegisVoice] No se pudo registrar Alt+Shift+A (tal vez en uso).")
            return

        msg = ctypes.wintypes.MSG() if hasattr(ctypes, "wintypes") else None
        if msg is None:
            import ctypes.wintypes as wt

            msg = wt.MSG()
        while user32.GetMessageW(ctypes.byref(msg), None, 0, 0) != 0:
            if msg.message == 0x0312:  # WM_HOTKEY
                on_trigger()
        user32.UnregisterHotKey(None, 1)

    threading.Thread(target=_loop, daemon=True).start()


def run_daemon(args: argparse.Namespace) -> None:
    try:
        import sounddevice as sd  # type: ignore
    except ImportError:
        print("Falta dependencia 'sounddevice'. Instalá con: pip install -r tools/desktop_voice/requirements.txt")
        sys.exit(1)

    token = args.token or os.environ.get("AEGIS_SESSION_TOKEN", "")
    passphrase = args.passphrase or os.environ.get("AEGIS_PASSWORD", "")
    if token:
        bridge = AegisHttpBridge(args.url, args.tenant, token)
    elif passphrase:
        bridge = AegisHttpBridge.login(args.url, args.tenant, passphrase)
    else:
        print("Configurá --token (o AEGIS_SESSION_TOKEN) o --passphrase (o AEGIS_PASSWORD).")
        sys.exit(1)

    vad = BreathAwareVadSegmenter(VadConfig(command_hangover_ms=args.pause_ms))
    audio_q: "queue.Queue[bytes]" = queue.Queue()
    speaking_lock = threading.Lock()
    followup_until = [0.0]

    def on_hotkey() -> None:
        print("[AegisVoice] Hotkey Alt+Shift+A activado — te escucho...")
        play_tone("ready")
        vad.arm_direct("direct")

    start_hotkey_listener(on_hotkey)
    print(f"[AegisVoice] Conectado a {args.url} (tenant={args.tenant}).")
    print('[AegisVoice] Decí "Aegis escuchame <pedido>" o presioná Alt+Shift+A aunque el navegador esté cerrado.')

    def callback(indata, frames, time_info, status) -> None:
        if speaking_lock.locked():
            return
        audio_q.put(bytes(indata))

    with sd.RawInputStream(
        samplerate=SAMPLE_RATE,
        blocksize=FRAME_SAMPLES,
        dtype="int16",
        channels=1,
        callback=callback,
    ):
        while True:
            frame = audio_q.get()
            if time.monotonic() < followup_until[0] and vad.mode == "wakeword":
                vad.mode = "followup"
            elif time.monotonic() >= followup_until[0] and vad.mode == "followup":
                vad.mode = "wakeword"

            result = vad.feed_frame(frame)
            if not result:
                continue

            pcm_bytes, mode = result
            try:
                res = bridge.post_utterance(pcm_bytes, mode, lang=args.lang)
            except urllib.error.URLError as exc:
                print(f"[AegisVoice] Servidor no disponible: {exc}")
                continue

            if not res.get("accepted"):
                if res.get("reason") == "speaker_mismatch":
                    play_tone("rejected")
                    print(f"[AegisVoice] Voz ignorada (score={res.get('score', 0):.2f})")
                continue

            speaker = res.get("matched_speaker") or "Autorizado"
            if res.get("wake_only"):
                play_tone("ready")
                print(f"[AegisVoice] ({speaker}) Escuchando tu pedido...")
                vad.arm_direct("direct")
                continue

            cmd = str(res.get("command", "")).strip()
            if not cmd:
                continue

            print(f"[AegisVoice] ({speaker}) -> {cmd}")
            with speaking_lock:
                reply = bridge.send_chat_and_wait(cmd)
                print(f"[AegisVoice] Aegis: {reply}")
                speak_text_native(reply)
                followup_until[0] = time.monotonic() + args.followup_secs


def main() -> None:
    parser = argparse.ArgumentParser(description="Aegis PC Always-On Voice Daemon")
    parser.add_argument("--url", default=os.environ.get("AEGIS_URL", "http://127.0.0.1:8000"))
    parser.add_argument("--tenant", default=os.environ.get("AEGIS_TENANT", "default"))
    parser.add_argument("--token", default="")
    parser.add_argument("--passphrase", default="")
    parser.add_argument("--lang", default="es-AR")
    parser.add_argument("--pause-ms", type=int, default=2400)
    parser.add_argument("--followup-secs", type=int, default=8)
    parser.add_argument("--install-startup", action="store_true")
    parser.add_argument("--uninstall-startup", action="store_true")
    args = parser.parse_args()

    if args.install_startup:
        install_windows_startup(args.url, args.tenant)
        return
    if args.uninstall_startup:
        uninstall_windows_startup()
        return

    run_daemon(args)


if __name__ == "__main__":
    main()
