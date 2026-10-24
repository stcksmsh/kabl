//! The factory presets under `patches/presets/` keep to docs/presets/BRIEF.md: one macro
//! module whose macros are named for what they do (on CC 20 and up, pinned in order), a
//! keyboard input, a category from the library vocabulary and a description. The level
//! rule is `factory_levels_hold_the_target` in `factory_bank.rs`, which renders every patch.

use std::path::Path;

use kabl_ui::library::{Meta, CATEGORIES};

#[test]
fn every_preset_keeps_to_the_brief() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/presets");
    let mut seen = 0;
    for dir in std::fs::read_dir(&root)
        .unwrap()
        .flatten()
        .map(|e| e.path())
    {
        let name = dir.file_name().unwrap().to_string_lossy().into_owned();
        let state = kabl_core::load(&dir).unwrap().state().clone();
        let meta: Meta =
            toml::from_str(&std::fs::read_to_string(dir.join("sound.toml")).unwrap()).unwrap();

        let macros: Vec<_> = state
            .modules
            .iter()
            .filter(|(_, m)| m.kind == "macro")
            .collect();
        assert_eq!(macros.len(), 1, "{name}: exactly one macro module");
        let (&id, module) = macros[0];
        // Each preset names its own macros after what they do: four is typical, more is
        // discouraged, and the module holds four. Named ones come first, on CC 20 and up.
        let named: Vec<String> = (1..=4)
            .map_while(|k| state.labels[&id].get(&format!("name.m{k}")).cloned())
            .collect();
        assert!(!named.is_empty(), "{name}: has named macros");
        for (k, label) in named.iter().enumerate() {
            let key = format!("m{}", k + 1);
            assert!(!label.trim().is_empty(), "{name}: macro {key} has a name");
            assert_eq!(
                module.params.get(&format!("cc.{key}")),
                Some(&(20.0 + k as f32)),
                "{name}: {label} is on CC {}",
                20 + k
            );
            assert!(
                module.params.contains_key(&format!("pin.{key}")),
                "{name}: {label} is pinned"
            );
            assert!(
                meta.description.contains(label.as_str()),
                "{name}: the description says what {label} does"
            );
        }
        assert!(meta.keys, "{name}: plays from MIDI");
        assert_eq!(
            (meta.keys, meta.sequence),
            Meta::play_of(&state),
            "{name}: play flags"
        );
        assert!(!meta.name.is_empty(), "{name}: has a name");
        assert!(
            CATEGORIES.contains(&meta.category.as_str()),
            "{name}: category"
        );
        assert!(!meta.tags.is_empty(), "{name}: has tags");
        assert!(meta.description.len() > 60, "{name}: has a description");
        seen += 1;
    }
    assert!(seen >= 8, "the pilot presets are there");
}
