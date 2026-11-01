//! Theme files: parsing, validation, the user themes folder and the remembered choice.
//!
//! A theme file is untrusted input. It is size-capped, parsed as TOML, and applied as an
//! overlay on a built-in theme, one value at a time: a value that is unknown, the wrong type
//! or out of range is dropped with a message and the base value stays. A mode whose text
//! would be unreadable is dropped whole. Nothing here can fail the app: the worst result is
//! the base theme plus notes.

use super::{
    builtin_a, builtin_b, compose, contrast, line_solid_default, outline_default, with_accent,
    Accent, Style, FACE_STYLES, TEXT_FLOOR,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Largest theme file read.
pub const MAX_THEME_BYTES: u64 = 64 * 1024;
/// Largest number of user theme files read.
pub const MAX_THEMES: usize = 64;
/// Vocabulary version a theme file may declare with `ui_api`.
pub const UI_API: i64 = 2;
/// Below this ratio a text pair is rejected; below 4.5 it is only reported.
const REJECT_BELOW: f32 = 3.0;

/// One theme: a light and a dark `Style`, plus what loading it had to say.
#[derive(Clone, Debug)]
pub struct Theme {
    pub id: String,
    pub name: String,
    pub light: Style,
    pub dark: Style,
    pub builtin: bool,
    pub notes: Vec<String>,
}

impl Theme {
    pub fn style(&self, dark: bool) -> &Style {
        if dark {
            &self.dark
        } else {
            &self.light
        }
    }
}

/// Every theme the app can switch to. The first entry is the default and the fallback.
#[derive(Clone, Debug)]
pub struct Themes {
    pub list: Vec<Theme>,
}

impl Default for Themes {
    fn default() -> Self {
        Themes {
            list: vec![
                builtin(builtin_a, "Satin Studio"),
                builtin(builtin_b, "Lacquer and Light"),
                builtin(line_solid_default, "Signal and Drafting solid"),
                builtin(outline_default, "Outline"),
            ],
        }
    }
}

fn builtin(f: fn(bool) -> Style, name: &str) -> Theme {
    let (light, dark) = (f(false), f(true));
    Theme {
        id: light.id.clone(),
        name: name.into(),
        light,
        dark,
        builtin: true,
        notes: Vec::new(),
    }
}

impl Themes {
    /// The built-ins plus every valid `*.toml` in `dir` (invalid files are kept out; their
    /// problems come back as messages).
    pub fn with_user_dir(dir: &Path) -> (Themes, Vec<String>) {
        let mut t = Themes::default();
        let mut msgs = Vec::new();
        let Ok(rd) = std::fs::read_dir(dir) else {
            return (t, msgs);
        };
        let mut files: Vec<PathBuf> = rd
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "toml") && p.is_file())
            .collect();
        files.sort();
        for p in files.into_iter().take(MAX_THEMES) {
            let shown = p.display();
            match read_capped(&p).and_then(|s| parse(&s, &t)) {
                Ok(th) if t.get(&th.id).is_some() => {
                    msgs.push(format!("{shown}: id {} already exists, skipped", th.id));
                }
                Ok(th) => {
                    msgs.extend(th.notes.iter().map(|n| format!("{shown}: {n}")));
                    t.list.push(th);
                }
                Err(e) => msgs.push(format!("{shown}: {e}")),
            }
        }
        (t, msgs)
    }

    pub fn get(&self, id: &str) -> Option<&Theme> {
        self.list.iter().find(|t| t.id == id)
    }

    /// The theme `id`, or the default when it is gone.
    pub fn resolve(&self, id: &str) -> &Theme {
        self.get(id).unwrap_or(&self.list[0])
    }
}

fn read_capped(p: &Path) -> Result<String, String> {
    use std::io::Read;
    let f = std::fs::File::open(p).map_err(|e| e.to_string())?;
    let mut s = String::new();
    f.take(MAX_THEME_BYTES + 1)
        .read_to_string(&mut s)
        .map_err(|e| e.to_string())?;
    if s.len() as u64 > MAX_THEME_BYTES {
        return Err(format!("larger than {MAX_THEME_BYTES} bytes"));
    }
    Ok(s)
}

/// Parse a theme file. `Err` only when the file is unusable as a theme at all (not TOML, no
/// id or name, unknown `extends`); every other problem becomes a note on the result.
pub fn parse(text: &str, known: &Themes) -> Result<Theme, String> {
    if text.len() as u64 > MAX_THEME_BYTES {
        return Err(format!("larger than {MAX_THEME_BYTES} bytes"));
    }
    let doc: toml::Table = toml::from_str(text).map_err(|e| format!("not valid TOML: {e}"))?;
    let id = doc
        .get("id")
        .and_then(toml::Value::as_str)
        .ok_or("missing `id`")?;
    if !valid_id(id) {
        return Err(format!(
            "bad id `{id}` (lowercase letters, digits, `.`, `-`, `_`; at most 64)"
        ));
    }
    let name = doc
        .get("name")
        .and_then(toml::Value::as_str)
        .filter(|n| !n.trim().is_empty() && n.chars().count() <= 48 && !n.contains('\n'))
        .ok_or("missing or bad `name` (1 to 48 characters)")?;
    let ext = doc
        .get("extends")
        .and_then(toml::Value::as_str)
        .unwrap_or(&known.list[0].id);
    let base = known
        .get(ext)
        .filter(|t| t.builtin)
        .ok_or_else(|| format!("`extends = \"{ext}\"` is not a built-in theme"))?;

    let mut notes = Vec::new();
    for k in doc.keys() {
        if ![
            "id",
            "name",
            "extends",
            "author",
            "version",
            "license",
            "ui_api",
            "light",
            "dark",
            "accent",
            "face_style",
        ]
        .contains(&k.as_str())
        {
            notes.push(format!("unknown key `{k}` ignored"));
        }
    }
    if doc
        .get("ui_api")
        .and_then(toml::Value::as_integer)
        .is_some_and(|v| v > UI_API)
    {
        notes.push("written for a newer kabl; unknown parts are ignored".into());
    }
    // ui_api 2: `accent` ("amber", "coral", "cyan" or "#rrggbb") and `face_style` choose two of
    // the three theme layers; a ui_api 1 file never reads them.
    let api = doc
        .get("ui_api")
        .and_then(toml::Value::as_integer)
        .unwrap_or(1);
    let (mut accent, mut face_style) = (None, None);
    if api >= 2 {
        if let Some(a) = doc.get("accent").and_then(toml::Value::as_str) {
            accent = Accent::parse(a);
            if accent.is_none() {
                notes.push(format!("accent `{a}` unknown, base accent kept"));
            }
        }
        if let Some(f) = doc.get("face_style").and_then(toml::Value::as_str) {
            face_style = FACE_STYLES.iter().copied().find(|s| *s == f);
            if face_style.is_none() {
                notes.push(format!("face_style `{f}` unknown, base face style kept"));
            }
        }
    } else if doc.contains_key("accent") || doc.contains_key("face_style") {
        notes.push("accent and face_style need ui_api = 2, ignored".into());
    }
    let mut mode = |dark: bool| {
        let key = if dark { "dark" } else { "light" };
        let layered = match (face_style, accent) {
            (Some(f), a) => compose(f, dark, a.unwrap_or(Accent::Amber)),
            (None, Some(a)) => Some(with_accent(base.style(dark).clone(), a)),
            (None, None) => None,
        };
        let b = layered.as_ref().unwrap_or_else(|| base.style(dark));
        match doc.get(key) {
            None => b.clone(),
            Some(toml::Value::Table(over)) => overlay(b, over, key, &mut notes),
            Some(_) => {
                notes.push(format!("`{key}` must be a table; base values used"));
                b.clone()
            }
        }
    };
    let (mut light, mut dark) = (mode(false), mode(true));
    for s in [&mut light, &mut dark] {
        s.id = id.into();
        s.name = name.into();
    }
    Ok(Theme {
        id: id.into(),
        name: name.into(),
        light,
        dark,
        builtin: false,
        notes,
    })
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
}

/// Apply `over` onto `base` value by value, then bound-check and contrast-check the result.
fn overlay(base: &Style, over: &toml::Table, mode: &str, notes: &mut Vec<String>) -> Style {
    let mut root = toml::Value::try_from(base).expect("a Style serialises");
    let mut path = Vec::new();
    walk(&mut root, over, &mut path, mode, notes);
    let Ok(mut s) = root.try_into::<Style>() else {
        notes.push(format!("{mode}: result unusable; base values used"));
        return base.clone();
    };
    s.metrics = base.metrics.clone();
    clamp(&mut s, base, mode, notes);
    for (n, fg, bg, min) in s.contrast_pairs() {
        let c = contrast(fg, bg);
        if c < REJECT_BELOW {
            notes.push(format!(
                "{mode}: {n} is {c:.1}:1, below {REJECT_BELOW:.1}; whole {mode} mode ignored, base values used"
            ));
            return base.clone();
        }
        if c < min {
            notes.push(format!(
                "{mode}: {n} is {c:.1}:1, below the {min:.1} target"
            ));
        }
    }
    s
}

fn node<'a>(root: &'a mut toml::Value, path: &[String]) -> Option<&'a mut toml::Value> {
    let mut v = root;
    for k in path {
        v = v.as_table_mut()?.get_mut(k)?;
    }
    Some(v)
}

fn walk(
    root: &mut toml::Value,
    over: &toml::Table,
    path: &mut Vec<String>,
    mode: &str,
    notes: &mut Vec<String>,
) {
    for (k, v) in over {
        path.push(k.clone());
        let shown = format!("{mode}.{}", path.join("."));
        // Identity and mode belong to the file, not to an overlay.
        if path.len() == 1 && ["id", "name", "dark"].contains(&k.as_str()) {
            notes.push(format!("{shown}: not themable here, ignored"));
        } else {
            match node(root, path) {
                None => notes.push(format!("{shown}: unknown key, ignored")),
                Some(cur) => match (cur.is_table(), v) {
                    (true, toml::Value::Table(t)) => walk(root, t, path, mode, notes),
                    _ => {
                        let old = std::mem::replace(cur, v.clone());
                        if root.clone().try_into::<Style>().is_err() {
                            *node(root, path).expect("just visited") = old;
                            notes.push(format!("{shown}: invalid value, base value kept"));
                        }
                    }
                },
            }
        }
        path.pop();
    }
}

/// Pull every number into its allowed range and keep text roles opaque. A value that is not
/// finite, or a text colour with transparency, goes back to the base.
fn clamp(s: &mut Style, base: &Style, mode: &str, notes: &mut Vec<String>) {
    fn num(v: &mut f32, lo: f32, hi: f32, dflt: f32, what: &str, mode: &str, n: &mut Vec<String>) {
        if !v.is_finite() {
            *v = dflt;
            n.push(format!("{mode}: {what} is not a number, base value used"));
        } else if *v < lo || *v > hi {
            *v = v.clamp(lo, hi);
            n.push(format!("{mode}: {what} limited to {lo} to {hi}"));
        }
    }
    let b = base;
    let types = [
        ("display", &mut s.types.display, &b.types.display),
        ("title", &mut s.types.title, &b.types.title),
        ("h3", &mut s.types.h3, &b.types.h3),
        ("body", &mut s.types.body, &b.types.body),
        ("label", &mut s.types.label, &b.types.label),
        ("value", &mut s.types.value, &b.types.value),
        ("caption", &mut s.types.caption, &b.types.caption),
        ("section", &mut s.types.section, &b.types.section),
    ];
    for (name, t, bt) in types {
        num(
            &mut t.size,
            TEXT_FLOOR,
            40.0,
            bt.size,
            &format!("type.{name}.size"),
            mode,
            notes,
        );
        num(
            &mut t.tracking,
            -2.0,
            8.0,
            bt.tracking,
            &format!("type.{name}.tracking"),
            mode,
            notes,
        );
    }
    for (i, v) in s.space.iter_mut().enumerate() {
        num(v, 0.0, 64.0, b.space[i], "space", mode, notes);
    }
    num(
        &mut s.radii.sm,
        0.0,
        24.0,
        b.radii.sm,
        "radii.sm",
        mode,
        notes,
    );
    num(
        &mut s.radii.md,
        0.0,
        24.0,
        b.radii.md,
        "radii.md",
        mode,
        notes,
    );
    num(
        &mut s.radii.lg,
        0.0,
        24.0,
        b.radii.lg,
        "radii.lg",
        mode,
        notes,
    );
    for (i, sh) in s.shadows.iter_mut().enumerate() {
        num(
            &mut sh.dy,
            -48.0,
            48.0,
            b.shadows[i].dy,
            "shadows.dy",
            mode,
            notes,
        );
        num(
            &mut sh.blur,
            0.0,
            48.0,
            b.shadows[i].blur,
            "shadows.blur",
            mode,
            notes,
        );
    }
    let m = &mut s.motion;
    for (v, d, what) in [
        (&mut m.hover, b.motion.hover, "motion.hover"),
        (&mut m.press, b.motion.press, "motion.press"),
        (&mut m.drawer, b.motion.drawer, "motion.drawer"),
        (&mut m.view, b.motion.view, "motion.view"),
        (&mut m.glide, b.motion.glide, "motion.glide"),
        (&mut m.pulse, b.motion.pulse, "motion.pulse"),
    ] {
        num(v, 0.0, 1200.0, d, what, mode, notes);
    }
    let sl = &mut s.slots;
    let bl = &b.slots;
    for (v, d, what) in [
        (&mut sl.face.gradient, bl.face.gradient, "face.gradient"),
        (&mut sl.face.gloss, bl.face.gloss, "face.gloss"),
        (&mut sl.face.grain, bl.face.grain, "face.grain"),
        (&mut sl.knob.glow, bl.knob.glow, "knob.glow"),
        (&mut sl.cable.glow, bl.cable.glow, "cable.glow"),
        (&mut sl.cable.dimmed, bl.cable.dimmed, "cable.dimmed"),
        (
            &mut sl.cable.rest_alpha,
            bl.cable.rest_alpha,
            "cable.rest_alpha",
        ),
        (&mut sl.display.glow, bl.display.glow, "display.glow"),
        (&mut sl.display.fill, bl.display.fill, "display.fill"),
        (
            &mut sl.controls.gradient,
            bl.controls.gradient,
            "controls.gradient",
        ),
    ] {
        num(v, 0.0, 1.0, d, what, mode, notes);
    }
    num(
        &mut sl.cable.width,
        1.0,
        12.0,
        bl.cable.width,
        "cable.width",
        mode,
        notes,
    );
    num(
        &mut sl.cable.rest_width,
        1.0,
        12.0,
        bl.cable.rest_width,
        "cable.rest_width",
        mode,
        notes,
    );
    num(
        &mut sl.cable.sag,
        0.0,
        200.0,
        bl.cable.sag,
        "cable.sag",
        mode,
        notes,
    );
    num(
        &mut sl.controls.outline,
        0.0,
        4.0,
        bl.controls.outline,
        "controls.outline",
        mode,
        notes,
    );
    num(
        &mut sl.controls.focus_ring,
        0.0,
        6.0,
        bl.controls.focus_ring,
        "controls.focus_ring",
        mode,
        notes,
    );
    sl.face.shadow = sl.face.shadow.min(2);
    sl.controls.card_shadow = sl.controls.card_shadow.min(2);
    sl.controls.popover_shadow = sl.controls.popover_shadow.min(2);
    // Text roles must be opaque, so a theme cannot make text vanish.
    let (r, br) = (&mut s.roles, &b.roles);
    for (c, d, what) in [
        (&mut r.text, br.text, "text"),
        (&mut r.text2, br.text2, "text2"),
        (&mut r.text3, br.text3, "text3"),
        (&mut r.on_accent, br.on_accent, "on_accent"),
        (&mut r.disp_text, br.disp_text, "disp_text"),
        (&mut r.knob_ink, br.knob_ink, "knob_ink"),
    ] {
        if c.a() != 255 {
            *c = d;
            notes.push(format!(
                "{mode}: roles.{what} must be opaque, base value used"
            ));
        }
    }
}

// ---- remembered choice ----

/// What is remembered across launches, in `settings.toml` of the config folder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Prefs {
    pub theme: String,
    pub dark: bool,
    /// The folder last used per place (`wavetables`, `patches`, `recordings`, `art`).
    #[serde(default)]
    pub folders: std::collections::BTreeMap<String, PathBuf>,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            theme: super::BUILTIN_A_ID.into(),
            dark: false,
            folders: Default::default(),
        }
    }
}

/// `KABL_CONFIG_DIR`, else `$XDG_CONFIG_HOME/kabl`, else `~/.config/kabl`.
pub fn config_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("KABL_CONFIG_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(d);
    }
    if let Some(d) = std::env::var_os("XDG_CONFIG_HOME").filter(|d| !d.is_empty()) {
        return PathBuf::from(d).join("kabl");
    }
    let home = std::env::var_os("HOME").map_or_else(|| PathBuf::from("."), PathBuf::from);
    home.join(".config/kabl")
}

pub fn themes_dir() -> PathBuf {
    config_dir().join("themes")
}

impl Prefs {
    /// The stored choice, or the default when absent or unreadable.
    pub fn load(dir: &Path) -> Prefs {
        read_capped(&dir.join("settings.toml"))
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Write via a temporary file so a crash cannot leave half a file.
    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let tmp = dir.join("settings.toml.tmp");
        std::fs::write(&tmp, toml::to_string(self).expect("prefs serialise"))?;
        std::fs::rename(tmp, dir.join("settings.toml"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn themes() -> Themes {
        Themes::default()
    }

    #[test]
    fn partial_theme_overrides_only_what_it_names() {
        let t = parse(
            "id = \"u.red\"\nname = \"Red\"\n[light.roles]\naccent = \"#c01010\"\n",
            &themes(),
        )
        .unwrap();
        let a = builtin_a(false);
        assert_eq!(t.light.roles.accent, Color::from_rgb(0xc0, 0x10, 0x10));
        assert_eq!(t.light.roles.bg, a.roles.bg);
        assert_eq!(t.dark.roles, builtin_a(true).roles);
        assert!(t.notes.is_empty(), "{:?}", t.notes);
    }

    type Color = egui::Color32;

    #[test]
    fn bad_values_fall_back_with_a_message() {
        let t = parse(
            "id = \"u.bad\"\nname = \"Bad\"\n[light.roles]\nbg = \"nope\"\nmystery = \"#fff\"\n\
             [light.slots.knob]\nbody = \"banana\"\nglow = 7.0\n[light.radii]\nsm = 99\n",
            &themes(),
        )
        .unwrap();
        let a = builtin_a(false);
        assert_eq!(t.light.roles.bg, a.roles.bg);
        assert_eq!(t.light.slots.knob.body, a.slots.knob.body);
        assert_eq!(t.light.slots.knob.glow, 1.0);
        assert_eq!(t.light.radii.sm, 24.0);
        let n = t.notes.join("\n");
        for want in [
            "roles.bg",
            "roles.mystery",
            "knob.body",
            "knob.glow",
            "radii.sm",
        ] {
            assert!(n.contains(want), "{want} not in {n}");
        }
    }

    #[test]
    fn unreadable_text_rejects_the_mode() {
        let t = parse(
            "id = \"u.dim\"\nname = \"Dim\"\n[light.roles]\ntext = \"#f3efe5\"\n",
            &themes(),
        )
        .unwrap();
        assert_eq!(
            t.light,
            Style {
                id: "u.dim".into(),
                name: "Dim".into(),
                ..builtin_a(false)
            }
        );
        assert!(t
            .notes
            .iter()
            .any(|n| n.contains("whole light mode ignored")));
    }

    #[test]
    fn translucent_text_and_type_floor_are_enforced() {
        let t = parse(
            "id = \"u.t\"\nname = \"T\"\n[dark.roles]\ntext = \"#ffffff40\"\n\
             [dark.types.caption]\nfont = \"sans\"\nsize = 6.0\n",
            &themes(),
        )
        .unwrap();
        assert_eq!(t.dark.roles.text, builtin_a(true).roles.text);
        assert_eq!(t.dark.types.caption.size, TEXT_FLOOR);
    }

    #[test]
    fn unusable_files_are_errors_not_panics() {
        for (text, why) in [
            ("not toml [", "TOML"),
            ("name = \"x\"", "id"),
            ("id = \"Bad Id\"\nname = \"x\"", "bad id"),
            ("id = \"u.a\"", "name"),
            ("id = \"u.a\"\nname = \"x\"\nextends = \"nope\"", "extends"),
        ] {
            let e = parse(text, &themes()).unwrap_err();
            assert!(e.contains(why), "{e}");
        }
        assert!(parse(&"#".repeat(70_000), &themes()).is_err());
    }

    #[test]
    fn user_dir_skips_bad_files_and_duplicate_ids() {
        let d = tempfile::tempdir().unwrap();
        let w = |n: &str, s: &str| std::fs::write(d.path().join(n), s).unwrap();
        w("a.toml", "id = \"u.a\"\nname = \"A2\"\n");
        w("b.toml", "garbage [");
        w("c.toml", "id = \"kabl.satin\"\nname = \"Clash\"\n");
        w("d.txt", "id = \"u.d\"\nname = \"D\"\n");
        std::fs::write(d.path().join("big.toml"), vec![b'#'; 70_000]).unwrap();
        let (t, msgs) = Themes::with_user_dir(d.path());
        let ids: Vec<_> = t.list.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "kabl.satin",
                "kabl.lacquer",
                "kabl.line_solid",
                "kabl.outline",
                "u.a"
            ]
        );
        assert_eq!(msgs.len(), 3, "{msgs:?}");
        assert_eq!(t.resolve("gone").id, "kabl.satin");
    }

    #[test]
    fn the_documented_example_theme_loads_clean() {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/themes/example/midnight-rose.toml");
        let t = parse(&std::fs::read_to_string(p).unwrap(), &themes()).unwrap();
        assert!(t.notes.is_empty(), "{:?}", t.notes);
        assert_eq!(t.dark.slots.knob.body, super::super::KnobBody::Cap);
        assert_eq!(t.dark.slots.knob.scale, super::super::KnobScale::TrackArc);
        assert_ne!(t.light, builtin_b(false));
    }

    #[test]
    fn prefs_round_trip_and_bad_file_gives_default() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(Prefs::load(d.path()), Prefs::default());
        let p = Prefs {
            theme: "kabl.lacquer".into(),
            dark: true,
            folders: [("patches".to_string(), PathBuf::from("/tmp"))].into(),
        };
        p.save(d.path()).unwrap();
        assert_eq!(Prefs::load(d.path()), p);
        // A file from before the folders were remembered still loads.
        std::fs::write(
            d.path().join("settings.toml"),
            "theme = \"kabl.lacquer\"\ndark = true\n",
        )
        .unwrap();
        assert_eq!(Prefs::load(d.path()).theme, "kabl.lacquer");
        assert!(Prefs::load(d.path()).folders.is_empty());
        std::fs::write(d.path().join("settings.toml"), "theme = 4").unwrap();
        assert_eq!(Prefs::load(d.path()), Prefs::default());
    }
}
