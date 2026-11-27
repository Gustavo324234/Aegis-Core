import io
import json
import os
import struct
import sys
import unittest
from unittest.mock import patch
import urllib.error

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from aegis_voice_daemon import (
    AegisHttpBridge,
    BreathAwareVadSegmenter,
    FRAME_SAMPLES,
    VadConfig,
    compute_rms_pcm16,
    format_http_error,
    parse_chat_ws_frame,
)


def make_pcm_frame(amplitude: int = 0) -> bytes:
    return struct.pack(f"<{FRAME_SAMPLES}h", *([amplitude] * FRAME_SAMPLES))


class BreathAwareVadTests(unittest.TestCase):
    def test_breath_pause_does_not_split_utterance_early(self) -> None:
        vad = BreathAwareVadSegmenter(
            VadConfig(command_hangover_ms=2400, wake_hangover_ms=1400, min_utterance_ms=300)
        )
        vad.arm_direct("direct", now_ms=0)
        silence = make_pcm_frame(0)
        voice = make_pcm_frame(6000)

        # Feed 4 frames of pre-roll silence
        for i in range(4):
            self.assertIsNone(vad.feed_frame(silence, now_ms=i * 30))

        # 600ms of first phrase
        for i in range(20):
            self.assertIsNone(vad.feed_frame(voice, now_ms=200 + i * 30))

        # 1.5s breathing pause -> must NOT flush yet (hangover is 2.4s)
        for i in range(50):
            res = vad.feed_frame(silence, now_ms=800 + i * 30)
            self.assertIsNone(res, "1.5s breath pause must not cut off speech")

        # User resumes speaking
        for i in range(15):
            self.assertIsNone(vad.feed_frame(voice, now_ms=2400 + i * 30))

        # Now real end-of-turn silence (> 2.4s)
        flushed = None
        t = 2900
        for _ in range(90):
            flushed = vad.feed_frame(silence, now_ms=t)
            if flushed is not None:
                break
            t += 30

        self.assertIsNotNone(flushed)
        payload, mode = flushed
        self.assertEqual(mode, "direct")
        # Payload must include pre-roll + both speech parts + breath pause
        self.assertGreater(len(payload), 80 * FRAME_SAMPLES * 2)

    def test_room_noise_never_triggers_utterance(self) -> None:
        vad = BreathAwareVadSegmenter()
        low_noise = make_pcm_frame(120)
        for i in range(200):
            self.assertIsNone(vad.feed_frame(low_noise, now_ms=i * 30))

    def test_compute_rms_pcm16_bounds(self) -> None:
        self.assertEqual(compute_rms_pcm16(b""), 0.0)
        rms = compute_rms_pcm16(make_pcm_frame(16384))
        self.assertAlmostEqual(rms, 0.5, places=2)

    def test_login_sends_session_key_and_extracts_session_token(self) -> None:
        captured = {}

        class _FakeResp(io.BytesIO):
            def __enter__(self):
                return self

            def __exit__(self, exc_type, exc, tb):
                return False

        def fake_urlopen(req, timeout=15):
            captured["url"] = req.full_url
            captured["body"] = json.loads(req.data.decode("utf-8"))
            payload = json.dumps(
                {
                    "status": "authenticated",
                    "role": "tenant",
                    "session_token": "cst_test_token_123",
                }
            ).encode("utf-8")
            return _FakeResp(payload)

        with patch("urllib.request.urlopen", side_effect=fake_urlopen):
            bridge = AegisHttpBridge.login("http://127.0.0.1:8000", "Tavo", "secret")

        self.assertEqual(captured["url"], "http://127.0.0.1:8000/api/auth/login")
        self.assertEqual(captured["body"], {"tenant_id": "Tavo", "session_key": "secret"})
        self.assertNotIn("passphrase", captured["body"])
        self.assertEqual(bridge.session_key, "cst_test_token_123")

    def test_parse_chat_ws_frame_handles_kernel_and_error_events(self) -> None:
        self.assertEqual(
            parse_chat_ws_frame({"event": "kernel_event", "data": {"pid": "p1", "output": "Hola"}}),
            ("Hola", False, None),
        )
        self.assertEqual(
            parse_chat_ws_frame(
                {"event": "kernel_event", "data": {"pid": "p1", "status_update": {"state": "STATE_COMPLETED"}}}
            ),
            (None, True, None),
        )
        self.assertEqual(
            parse_chat_ws_frame({"event": "kernel_event", "data": {"pid": "p1", "error": "boom"}}),
            (None, True, "boom"),
        )
        self.assertEqual(
            parse_chat_ws_frame({"event": "error", "data": {"code": "E_RATE", "message": "Rate limited"}}),
            (None, True, "Rate limited"),
        )

    def test_format_http_error_extracts_json_server_reason(self) -> None:
        err = urllib.error.HTTPError(
            "http://127.0.0.1:8000/api/siren/utterance",
            400,
            "Bad Request",
            {},
            io.BytesIO(json.dumps({"error": "No hay motor STT disponible"}).encode("utf-8")),
        )
        self.assertEqual(format_http_error(err), "HTTP 400: No hay motor STT disponible")
        err.close()


if __name__ == "__main__":
    unittest.main()
