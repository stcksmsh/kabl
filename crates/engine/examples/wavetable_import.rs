//! Embeds a .wav file in a saved patch as a user wavetable. Control-thread work only; the patch
//! keeps the table itself, so the .wav can be deleted afterwards.
//!
//!     cargo run --release -p kabl-engine --example wavetable_import -- PATCH_DIR SLOT FILE.wav
//!
//! SLOT is 1-8: a `osc.wt` module with `user` set to that number plays it. A file whose length
//! is a multiple of 2048 samples is that many frames (up to 64); any other length is one cycle,
//! resampled. A `clm ` chunk (Serum style) names another frame length.

use kabl_core::{Op, Source, Table};
use kabl_modules::wavetable;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [dir, slot, file] = args.as_slice() else {
        return Err("usage: wavetable_import PATCH_DIR SLOT FILE.wav".into());
    };
    let slot: u64 = slot.parse().map_err(|_| "SLOT must be 1-8")?;
    let (name, wav) = wavetable::import_file(std::path::Path::new(file))?;
    let mut log =
        kabl_core::load(std::path::Path::new(dir)).map_err(|e| format!("{dir}: {e:?}"))?;
    let mut candidate = log.state().clone();
    candidate.tables.insert(
        slot,
        Table {
            name: name.clone(),
            wav: wav.clone(),
        },
    );
    kabl_core::composite::validate(&candidate)?;
    let size = serde_json::to_vec(&candidate)
        .map_err(|e| e.to_string())?
        .len();
    if size > 2 * 1024 * 1024 - 16 * 1024 {
        return Err("the sound would be too large to save with this table".into());
    }
    let frames = wavetable::decode_canonical(&wav).map_or(0, |f| f.len());
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);
    log.append(
        Op::SetTable {
            slot,
            value: Some(Table {
                name: name.clone(),
                wav,
            }),
        },
        now,
        Source::User,
    );
    kabl_core::save(std::path::Path::new(dir), &log).map_err(|e| format!("{dir}: {e:?}"))?;
    println!("slot {slot}: {name} ({frames} frames)");
    Ok(())
}
