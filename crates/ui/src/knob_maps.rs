//! Built-in macro knob maps for common MIDI controllers, matched by input port name.
//!
//! Each map is the controller's factory knob CCs in the mode the user must select. Sources are
//! the maker's own document, or Spectrasonics' Omnisphere hardware page for the device's factory
//! template (research of 2026-11-27). Makers rarely document the channel, so a map takes its
//! CCs on any channel, and only absolute values.

/// One controller family's knob map.
#[derive(Debug, PartialEq)]
pub struct Controller {
    pub name: &'static str,
    /// Port-name token sets: the port matches when every token of one set is in its name.
    pub ports: &'static [&'static [&'static str]],
    /// Knob 1.. CCs, driving m1.. in order.
    pub ccs: &'static [u8],
    /// What the user selects on the device so its knobs send these CCs.
    pub mode: &'static str,
    pub source: &'static str,
}

/// Spectrasonics' page for a device's factory template.
macro_rules! omni {
    ($slug:literal) => {
        concat!(
            "https://support.spectrasonics.net/manual/Omnisphere3HW/3/en/topic/setting-up-the-",
            $slug
        )
    };
}

const ARTURIA: [u8; 8] = [74, 71, 76, 77, 93, 18, 19, 16];
const NOVATION: [u8; 8] = [21, 22, 23, 24, 25, 26, 27, 28];

pub const CONTROLLERS: &[Controller] = &[
    Controller {
        name: "Arturia KeyLab Essential mk3",
        ports: &[&["kl", "essential", "mk3"], &["keylab", "essential", "mk3"]],
        ccs: &ARTURIA,
        mode: "press Prog until User 1",
        source: omni!("keylab-essential-49-mk3"),
    },
    Controller {
        name: "Arturia KeyLab mk3",
        ports: &[&["kl", "mk3"], &["keylab", "mk3"]],
        ccs: &ARTURIA,
        mode: "press Prog until User 1",
        source: omni!("keylab-61-mk3"),
    },
    Controller {
        name: "Arturia KeyLab mkII",
        ports: &[&["keylab", "mkii"]],
        ccs: &ARTURIA,
        mode: "press User for the User preset",
        source: omni!("keylab-49-mk2"),
    },
    Controller {
        name: "Arturia MiniLab 3",
        ports: &[&["minilab", "3"]],
        ccs: &ARTURIA,
        mode: "Shift + Prog pad until User 1",
        source: omni!("minilab-mk3"),
    },
    // Knob 1 is reserved for Analog Lab and its CC is not documented: knobs 2-8 drive m1-m7.
    Controller {
        name: "Arturia MiniLab mkII",
        ports: &[&["minilab", "mkii"]],
        ccs: &[74, 71, 76, 77, 93, 73, 75],
        mode: "Shift + pad 1 for User preset 1; knobs 2-8",
        source: omni!("minilab-mk2"),
    },
    Controller {
        name: "Novation Launchkey MK3",
        ports: &[&["launchkey", "mk3"]],
        ccs: &NOVATION,
        mode: "Shift + Custom 1 pad (Custom pot mode)",
        source: omni!("launchkey-49-mk3"),
    },
    Controller {
        name: "Novation Launchkey Mini MK3",
        ports: &[&["launchkey", "mini", "mk3"]],
        ccs: &NOVATION,
        mode: "Shift + Custom pad (Custom pot mode)",
        source: omni!("launchkey-mini-mk3"),
    },
    Controller {
        name: "Novation Launchkey MK4",
        ports: &[&["launchkey", "mk4"]],
        ccs: &NOVATION,
        mode: "Shift + pad 5 (encoders Custom 1)",
        source: omni!("launchkey-49-mk4"),
    },
    Controller {
        name: "Novation Launchkey Mini MK4",
        ports: &[&["launchkey", "mini", "mk4"]],
        ccs: &NOVATION,
        mode: "Shift + Custom 1 (Custom pot mode)",
        source: omni!("launchkey-mini-mk4"),
    },
    // The mk2's port name ends in its device id, so "3" cannot tell the XL 3 apart: both share
    // the top-row CCs.
    Controller {
        name: "Novation Launch Control XL",
        ports: &[&["launch", "control", "xl"]],
        ccs: &[13, 14, 15, 16, 17, 18, 19, 20],
        mode: "User + pad 1 for User template 1 (XL 3: Custom mode 1)",
        source: concat!(
            omni!("launch-control-xl-mk2"),
            " ; ",
            omni!("launch-control-xl-mk3")
        ),
    },
    Controller {
        name: "Akai MPK Mini mk3",
        ports: &[&["mpk", "mini", "3"]],
        ccs: &[70, 71, 72, 73, 74, 75, 76, 77],
        mode: "Prog Select + pad 1 (PGM 1: MPC)",
        source: omni!("mpk-mini-mk3"),
    },
    Controller {
        name: "Akai MPK Mini Plus",
        ports: &[&["mpk", "mini", "plus"]],
        ccs: &[70, 71, 72, 73, 74, 75, 76, 77],
        mode: "Program 1 (PGM 1: MPC)",
        source: omni!("mpk-mini-plus"),
    },
    Controller {
        name: "Akai MPK249",
        ports: &[&["mpk249"]],
        ccs: &[83, 85, 86, 87, 88, 89, 90, 91],
        mode: "Preset 30 Generic, Control Bank C",
        source: omni!("mpk249"),
    },
    Controller {
        name: "Akai MPK261",
        ports: &[&["mpk261"]],
        ccs: &[83, 85, 86, 87, 88, 89, 90, 91],
        mode: "Preset 30 Generic, Control Bank C",
        source: omni!("mpk261"),
    },
    Controller {
        name: "Akai MPD218",
        ports: &[&["mpd218"]],
        ccs: &[3, 9, 12, 13, 14, 15],
        mode: "factory Program 1",
        source: omni!("mpd218"),
    },
    Controller {
        name: "Akai MPD226",
        ports: &[&["mpd226"]],
        ccs: &[3, 9, 14, 15],
        mode: "Preset 1 Generic",
        source: omni!("mpd226"),
    },
    Controller {
        name: "Native Instruments Komplete Kontrol A",
        ports: &[
            &["komplete", "kontrol", "a25"],
            &["komplete", "kontrol", "a49"],
            &["komplete", "kontrol", "a61"],
        ],
        ccs: &[14, 15, 16, 17, 18, 19, 20, 21],
        mode: "Shift + Plug-in/MIDI (MIDI mode)",
        source: concat!(
            omni!("komplete-kontrol-a49"),
            " ; ",
            "https://support.native-instruments.com/support/solutions/articles/69000879684"
        ),
    },
    // Maker-documented: page 1 knobs, channel 1.
    Controller {
        name: "Native Instruments Kontrol S mk3",
        ports: &[
            &["kontrol", "s25", "mk3"],
            &["kontrol", "s49", "mk3"],
            &["kontrol", "s61", "mk3"],
            &["kontrol", "s88", "mk3"],
        ],
        ccs: &[14, 15, 16, 17, 18, 19, 20, 21],
        mode: "Shift + Settings (MIDI mode)",
        source:
            "https://docs.native-instruments.com/online-guides/kontrol-s-mk3-manual/en/midi-mode",
    },
    Controller {
        name: "M-Audio Oxygen Pro",
        ports: &[
            &["oxygen", "pro", "25"],
            &["oxygen", "pro", "49"],
            &["oxygen", "pro", "61"],
        ],
        ccs: &[44, 45, 46, 47, 62, 63, 75, 76],
        mode: "DAW/Preset to Preset, Preset 16",
        source: omni!("oxygen-pro-49"),
    },
    Controller {
        name: "M-Audio Keystation Pro 88",
        ports: &[&["keystation", "pro", "88"]],
        ccs: &[10, 2, 12, 13, 75, 76, 92, 95],
        mode: "01 GM Preset",
        source: omni!("keystation-pro-88"),
    },
    // Maker-documented: native mode, channel 16.
    Controller {
        name: "Korg nanoKONTROL2",
        ports: &[&["nanokontrol2"]],
        ccs: &[16, 17, 18, 19, 20, 21, 22, 23],
        mode: "factory native mode",
        source:
            "https://cdn.korg.com/us/support/download/files/aeb2862daf0cb7db826d8c62f51ec28d.txt",
    },
    Controller {
        name: "Nektar Impact LX mk3",
        ports: &[
            &["impact", "lx25", "mk3"],
            &["impact", "lx49", "mk3"],
            &["impact", "lx61", "mk3"],
            &["impact", "lx88", "mk3"],
        ],
        ccs: &[89, 90, 96, 97, 116, 117, 118, 119],
        mode: "hold Preset + turn to preset 5 Learn Friendly 2",
        source: omni!("impact-lx49-mk3"),
    },
    Controller {
        name: "Alesis V mkII",
        ports: &[&["v25", "mkii"], &["v49", "mkii"], &["v61", "mkii"]],
        ccs: &[74, 75, 76, 77],
        mode: "factory preset",
        source: omni!("v49-mk2"),
    },
    Controller {
        name: "Alesis VI",
        ports: &[&["vi49"], &["vi61"]],
        ccs: &[20, 21, 22, 23, 24, 25, 26, 27],
        mode: "factory preset",
        source: omni!("vi49"),
    },
    Controller {
        name: "Roland A-300PRO",
        ports: &[&["a", "300pro"], &["a300pro"]],
        ccs: &[16, 17, 18, 19, 80, 81, 82, 83],
        mode: "Control Map 18 (CC Control Temp)",
        source: omni!("a-300pro"),
    },
];

/// Port-name tokens of a multi-port controller's other ports (DAW scripts, Mackie, thru): its
/// knobs send nothing plain there.
const NOT_KNOB_PORT: &[&str] = &[
    "mcu",
    "hui",
    "alv",
    "dinthru",
    "thru",
    "daw",
    "plugin",
    "software",
    "incontrol",
    "midiin2",
];

/// The built-in map of the controller on this port, the most specific when several match.
/// Tokens are case-insensitive; a trailing ALSA " client:port" id is ignored.
pub fn find(port: &str) -> Option<&'static Controller> {
    let port = port.trim().to_lowercase();
    let name = match port.rsplit_once(' ') {
        Some((head, id)) if is_alsa_id(id) => head,
        _ => &port,
    };
    let tokens: Vec<&str> = name
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .collect();
    if tokens.iter().any(|t| NOT_KNOB_PORT.contains(t)) {
        return None;
    }
    CONTROLLERS
        .iter()
        .filter_map(|c| {
            let best = c
                .ports
                .iter()
                .filter(|set| set.iter().all(|t| tokens.contains(t)))
                .map(|set| set.len())
                .max()?;
            Some((best, c))
        })
        .max_by_key(|&(n, _)| n)
        .map(|(_, c)| c)
}

fn is_alsa_id(id: &str) -> bool {
    id.split_once(':').is_some_and(|(a, b)| {
        [a, b]
            .iter()
            .all(|s| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_map_has_one_to_eight_distinct_ccs_a_mode_and_a_source() {
        for c in CONTROLLERS {
            assert!((1..=8).contains(&c.ccs.len()), "{}", c.name);
            assert!(c.ccs.iter().all(|&cc| cc < 128), "{}", c.name);
            let mut ccs = c.ccs.to_vec();
            ccs.sort();
            ccs.dedup();
            assert_eq!(ccs.len(), c.ccs.len(), "{} repeats a CC", c.name);
            assert!(!c.mode.is_empty(), "{}", c.name);
            assert!(c.source.starts_with("https://"), "{}", c.name);
            assert!(!c.ports.is_empty() && c.ports.iter().all(|s| !s.is_empty()));
        }
    }

    fn name(port: &str) -> Option<&'static str> {
        find(port).map(|c| c.name)
    }

    #[test]
    fn keylab_essential_mk3_matches_its_midi_port_in_every_spelling() {
        let want = Some("Arturia KeyLab Essential mk3");
        assert_eq!(name("KL Essential 61 mk3 MIDI"), want);
        assert_eq!(name("KL Essential 49 mk3 MIDI 24:0"), want);
        assert_eq!(
            name("KL Essential 61 mk3 KL Essential 61 mk3 MIDI 24:0"),
            want
        );
        assert_eq!(name("KeyLab Essential mk3 MIDI"), want);
        assert_eq!(name("kl essential 88 MK3 midi"), want);
    }

    #[test]
    fn a_controllers_other_ports_match_nothing() {
        for port in [
            "KL Essential 61 mk3 MCU/HUI",
            "KL Essential 61 mk3 ALV",
            "KL Essential 61 mk3 DINTHRU",
            "MiniLab 3 DIN Thru",
            "Launchkey MK3 49 LKMK3 DAW Out",
            "MIDIIN2 (Launchkey Mini MK3)",
            "Launchkey Mini MK3 InControl",
            "MPK Mini IV Plugin Port",
            "MPK Mini IV Software Control Port",
            "Pad 24:0",
            "",
        ] {
            assert_eq!(name(port), None, "{port}");
        }
    }

    #[test]
    fn keylab_essential_mk3_beats_keylab_mk3() {
        assert_eq!(
            name("KL Essential 61 mk3 MIDI"),
            Some("Arturia KeyLab Essential mk3")
        );
        assert_eq!(name("KL mk3 MIDI"), Some("Arturia KeyLab mk3"));
        assert_eq!(name("KeyLab 61 mk3 MIDI"), Some("Arturia KeyLab mk3"));
    }

    #[test]
    fn launchkey_mini_beats_launchkey() {
        assert_eq!(
            name("Launchkey Mini MK3 Launchkey Mini MK3 MIDI 20:0"),
            Some("Novation Launchkey Mini MK3")
        );
        assert_eq!(
            name("Launchkey MK3 49 LKMK3 MIDI Out"),
            Some("Novation Launchkey MK3")
        );
        assert_eq!(
            name("Launchkey Mini MK4 25 MIDI"),
            Some("Novation Launchkey Mini MK4")
        );
        assert_eq!(
            name("Launchkey MK4 49 MIDI"),
            Some("Novation Launchkey MK4")
        );
    }

    #[test]
    fn minilab_3_and_minilab_mkii_stay_apart() {
        assert_eq!(name("MiniLab 3 MIDI"), Some("Arturia MiniLab 3"));
        assert_eq!(name("Arturia MiniLab mkII"), Some("Arturia MiniLab mkII"));
        // The ALSA id's port number is not the model's "3".
        assert_eq!(
            name("Arturia MiniLab mkII 20:3"),
            Some("Arturia MiniLab mkII")
        );
        assert_eq!(name("Arturia MiniLab 20:3"), None);
    }
}
