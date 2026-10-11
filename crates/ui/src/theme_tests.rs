use skrifa::{FontRef, MetadataProvider};

/// Every non-ASCII character in the interface source must have a glyph in the embedded
/// fonts: egui's own default fonts are off, so a gap would draw as a missing-glyph box.
#[test]
fn embedded_fonts_cover_every_character_the_interface_draws() {
    let mut chars = std::collections::BTreeSet::new();
    let mut dirs = vec![std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")];
    while let Some(d) = dirs.pop() {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                dirs.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                for l in std::fs::read_to_string(&p).unwrap().lines() {
                    if !l.trim_start().starts_with("//") {
                        chars.extend(l.chars().filter(|c| !c.is_ascii()));
                    }
                }
            }
        }
    }
    assert!(chars.len() > 10, "scan found too few characters");
    let fonts: [(&str, &[u8]); 5] = [
        ("sans", include_bytes!("../assets/IBMPlexSans-Regular.ttf")),
        ("sans-medium", include_bytes!("../assets/IBMPlexSans-Medium.ttf")),
        ("sans-semibold", include_bytes!("../assets/IBMPlexSans-SemiBold.ttf")),
        ("mono", include_bytes!("../assets/IBMPlexMono-Regular.ttf")),
        ("mono-medium", include_bytes!("../assets/IBMPlexMono-Medium.ttf")),
    ];
    for (name, bytes) in fonts {
        let map = FontRef::new(bytes).unwrap().charmap();
        let missing: String = chars.iter().filter(|&&c| map.map(c).is_none()).collect();
        assert!(missing.is_empty(), "{name} lacks {missing:?}");
    }
}
