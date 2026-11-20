//! The first-run start patch candidates, rendered for the listening gate (Kosta's gate D): 20 s
//! of each, from a fresh start with nobody touching anything.
//!
//!     cargo run --release -p kabl-tools --example first_run_candidates -- OUT_DIR
//!
//! Writes `OUT_DIR/<id>.mp3` (needs `ffmpeg`) and prints peak, RMS and how much of the clip
//! sounds, per candidate. Offline: no audio device is opened.
use kabl_ui::first_run;

const RATE: f32 = 48000.0;
const SECS: f32 = 20.0;

fn main() {
    let dir = std::path::PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "docs/first-run".into()),
    );
    std::fs::create_dir_all(&dir).unwrap();
    for (i, c) in first_run::candidates().iter().enumerate() {
        let state = c.build().state().clone();
        let audio = first_run::render(&state, SECS, RATE).expect("renders");
        let peak = audio.iter().fold(0f32, |m, s| m.max(s.abs()));
        let win = (RATE as usize / 20) * 2;
        let (mut sum, mut n, mut sounding, mut windows) = (0f64, 0usize, 0usize, 0usize);
        for w in audio.chunks(win) {
            let ms = w.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / w.len() as f64;
            windows += 1;
            if 10.0 * ms.max(1e-20).log10() > -50.0 {
                sounding += 1;
            }
            sum += ms;
            n += 1;
        }
        let rms_db = 10.0 * (sum / n.max(1) as f64).log10();
        // Does it move? Loudness and brightness over 100 ms windows, 5th to 95th percentile:
        // a pulse shows as a wide loudness swing, the cutoff opening as a wide brightness swing
        // (the share of energy in the first difference, a cheap high-frequency measure).
        let slice = (RATE as usize / 10) * 2;
        let (mut loud, mut bright) = (Vec::new(), Vec::new());
        for w in audio.chunks(slice).skip(5) {
            let e: f64 = w.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() + 1e-20;
            let d: f64 = w.windows(2).map(|p| ((p[1] - p[0]) as f64).powi(2)).sum();
            loud.push(10.0 * (e / w.len() as f64).log10());
            bright.push(10.0 * (d / e + 1e-9).log10());
        }
        let spread = |v: &mut Vec<f64>| {
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            v[v.len() * 95 / 100] - v[v.len() * 5 / 100]
        };
        println!(
            "{}{} {:<11} peak {:>6.1} dBFS  rms {:>6.1} dB  sounding {:>3.0} %  loudness swing {:>4.1} dB  brightness swing {:>4.1} dB  {}",
            if i == first_run::START { "*" } else { " " },
            i + 1,
            c.id,
            20.0 * peak.max(1e-9).log10(),
            rms_db,
            100.0 * sounding as f64 / windows as f64,
            spread(&mut loud),
            spread(&mut bright),
            c.name,
        );
        let wav = dir.join(format!("{}.wav", c.id));
        let mut w = hound::WavWriter::create(
            &wav,
            hound::WavSpec {
                channels: 2,
                sample_rate: RATE as u32,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for s in &audio {
            w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16)
                .unwrap();
        }
        w.finalize().unwrap();
        let mp3 = dir.join(format!("{}.mp3", c.id));
        let ok = std::process::Command::new("ffmpeg")
            .args(["-y", "-loglevel", "error", "-i"])
            .arg(&wav)
            .args(["-codec:a", "libmp3lame", "-q:a", "3"])
            .arg(&mp3)
            .status()
            .is_ok_and(|s| s.success());
        if ok {
            let _ = std::fs::remove_file(&wav);
        } else {
            println!("  ffmpeg failed: kept {}", wav.display());
        }
    }
}
