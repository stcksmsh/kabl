//! Offline envelope-time A/B renders for the revision-2 prototype. Writes WAV files; opens no
//! audio device. Only the timing policy differs between each A/B pair.
//!
//!     cargo run -p kabl-ui --example rev2_env_ab --release [-- <out dir>]

#[path = "rev2_proto/envtime.rs"]
#[allow(dead_code)]
mod envtime;

use envtime::{render, rms_diff, write_wav, Policy, Scenario};

fn main() -> std::io::Result<()> {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or("target/rev2-proto/env-ab".into()));
    std::fs::create_dir_all(&dir)?;
    // The reference patch (Attack 8 ms, LFO 0.8 Hz TRI, +25 %), the same with a deeper route,
    // and a slow pad where the policies should differ most.
    let mut deep = Scenario::from_patch(8.0, 0.8, 1, 0.5);
    deep.name = "patch-deep";
    let scenarios = [Scenario::from_patch(8.0, 0.8, 1, 0.25), deep, Scenario::slow_pad()];
    for sc in &scenarios {
        let a = render(sc, Policy::Continuous);
        let b = render(sc, Policy::StageStart);
        write_wav(&dir.join(format!("{}-A-continuous.wav", sc.name)), &a.audio)?;
        write_wav(&dir.join(format!("{}-B-stage-start.wav", sc.name)), &b.audio)?;
        println!(
            "{:<11} attack {:>6.1} ms  lfo {:.2} Hz {:?}  amount {:+.0} %  rms diff env {:.4} audio {:.4}",
            sc.name,
            sc.attack_ms,
            sc.lfo_hz,
            sc.lfo_wave,
            sc.amount * 100.0,
            rms_diff(&a.env, &b.env),
            rms_diff(&a.audio, &b.audio)
        );
        let ms = |v: &Option<f32>| v.map(|s| format!("{:.0}", s * 1000.0)).unwrap_or("-".into());
        println!("  rise to 0.9 (ms), A: {}", a.rise_90.iter().map(ms).collect::<Vec<_>>().join(" "));
        println!("  rise to 0.9 (ms), B: {}", b.rise_90.iter().map(ms).collect::<Vec<_>>().join(" "));
    }
    println!("wrote {}", dir.display());
    Ok(())
}
