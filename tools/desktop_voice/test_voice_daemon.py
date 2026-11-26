import os
import struct
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from aegis_voice_daemon import (
    BreathAwareVadSegmenter,
    FRAME_SAMPLES,
    VadConfig,
    compute_rms_pcm16,
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


if __name__ == "__main__":
    unittest.main()
