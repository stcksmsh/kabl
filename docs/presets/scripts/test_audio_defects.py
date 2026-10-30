#!/usr/bin/env python3
"""Self-test of audio_defects.py on synthetic signals: python3 docs/presets/scripts/test_audio_defects.py"""
import sys, tempfile, unittest
from pathlib import Path
import numpy as np
import soundfile as sf

sys.path.insert(0, str(Path(__file__).resolve().parent))
import audio_defects as ad

SR = 48000
t = np.arange(SR * 4) / SR


def pad():
    """A clean two-partial pad with a slow swell, stereo, -12 dBFS."""
    m = 0.18 * np.sin(2 * np.pi * 220 * t) + 0.1 * np.sin(2 * np.pi * 331 * t + 1)
    m *= 0.7 + 0.3 * np.sin(2 * np.pi * 0.5 * t)
    return np.stack([m, m * 0.9], 1)


def scan(x, name='a.wav'):
    with tempfile.TemporaryDirectory() as d:
        sf.write(f'{d}/{name}', x, SR, subtype='FLOAT')
        return ad.scan(Path(d) / name)['flags']


class Detector(unittest.TestCase):
    def test_clean_pad_has_no_flags(self):
        self.assertEqual(scan(pad()), {})

    def test_single_sample_spike_is_a_click(self):
        x = pad()
        x[2 * SR, 0] += 0.2
        f = scan(x)
        self.assertIn('CLICK', f)
        self.assertAlmostEqual(f['CLICK'][0], 2.0, delta=0.06)

    def test_smooth_attack_is_not_a_click(self):
        x = pad()
        x[:SR] = 0
        x[SR:SR + 48] *= np.linspace(0, 1, 48)[:, None]  # 1 ms fade-in
        self.assertNotIn('CLICK', scan(x))

    def test_attack_at_the_start_of_a_window_is_not_a_click(self):
        # A kick whose short attack begins exactly on a 50 ms window boundary, after a noise
        # burst: the aligned window holds nothing but the attack and the tail.
        x = np.zeros((SR * 4, 2))
        x[SR:2 * SR] = np.random.default_rng(1).uniform(-0.2, 0.2, (SR, 1))
        n = 3
        k = 0.5 * np.cos(2 * np.pi * 120 * t[:SR]) * np.exp(-t[:SR] * 20)
        k[:n] *= 0.5 - 0.5 * np.cos(np.pi * np.arange(n) / n)
        x[2 * SR:3 * SR] = k[:, None]
        self.assertNotIn('CLICK', scan(x))

    def test_hard_clipped_signal_is_pinned(self):
        x = np.clip(pad() * 6, -0.84, 0.84)
        self.assertIn('PINNED', scan(x))

    def test_peak_over_minus_1_dbfs(self):
        x = pad() * 0.8 / 0.28  # ~ -1.9 dBFS ok, then over
        self.assertNotIn('PEAK', scan(x * 0.8))
        self.assertIn('PEAK', scan(np.clip(x * 1.5, -1, 1)))

    def test_click_in_lsb_loud_window_is_ignored(self):
        x = np.zeros((SR, 2))
        x[SR // 2, :] = 8 / 32768
        self.assertNotIn('CLICK', scan(x))

    def test_dc_offset(self):
        self.assertIn('DC', scan(pad() + 0.05))

    def test_nan_is_reported(self):
        x = pad()
        x[1000, 1] = np.nan
        self.assertIn('NONFINITE', scan(x))

    def test_denormal_in_float_wav(self):
        x = pad()
        x[1000, 0] = 1e-40
        self.assertIn('DENORMAL', scan(x))

    def test_hf_burst_without_band_rise(self):
        x = pad()
        n = np.arange(240)
        x[2 * SR:2 * SR + 240, 0] += 0.05 * np.sin(2 * np.pi * 9000 * n / SR)
        self.assertIn('HF_BURST', scan(x))


if __name__ == '__main__':
    unittest.main()
