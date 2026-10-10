//! Token sheet generator and contrast checker.
use crate::prim::contrast;
use crate::tokens::*;

pub fn check() {
    for dir in Dir::ALL {
        for dark in [false, true] {
            let k = tok(dir, dark);
            let mut rows: Vec<(String, f32, f32)> = vec![];
            let mut add =
                |n: &str, fg, bg, min: f32| rows.push((n.to_string(), contrast(fg, bg), min));
            add("text/surface", k.text, k.surface, 4.5);
            add("text/bg", k.text, k.bg, 4.5);
            add("text2/surface", k.text2, k.surface, 4.5);
            add("text2/inset", k.text2, k.inset, 4.5);
            add("text3/surface (hint)", k.text3, k.surface, 3.0);
            add("on_accent/accent", k.on_accent, k.accent, 4.5);
            add("accent/surface (non-text)", k.accent, k.surface, 3.0);
            add("disp_text/disp_bg", k.disp_text, k.disp_bg, 4.5);
            add("disp_trace/disp_bg", k.disp_trace, k.disp_bg, 3.0);
            add("knob_ink/knob", k.knob_ink, k.knob, 4.5);
            for s in Sect::ALL {
                let f = k.sect(s);
                add(&format!("face ink/{}", s.name()), f.ink, f.base, 4.5);
                add(&format!("face ink2/{}", s.name()), f.ink2, f.base, 4.5);
            }
            println!("== {} {}", dir.title(), if dark { "dark" } else { "light" });
            for (n, c, m) in rows {
                let verdict = if c >= m {
                    "ok".to_string()
                } else {
                    format!("FAIL (min {m})")
                };
                println!("  {:<42} {:>5.2}  {}", n, c, verdict);
            }
        }
    }
}

fn hex(c: egui::Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

pub fn tokens_md() {
    for dir in Dir::ALL {
        println!("# Token sheet — {}\n", dir.title());
        for dark in [false, true] {
            let k = tok(dir, dark);
            println!("## {} theme\n", if dark { "Dark" } else { "Light" });
            println!("### Colour\n\n| token | value | use |\n|---|---|---|");
            let rows: [(&str, egui::Color32, &str); 29] = [
                ("bg", k.bg, "window background"),
                ("surface", k.surface, "toolbar, panels, status bar"),
                ("raised", k.raised, "cards, popovers, active segment"),
                ("inset", k.inset, "fields, wells, tracks"),
                ("line", k.line, "hairlines"),
                ("line2", k.line2, "strong rules, outlines"),
                ("text", k.text, "primary text"),
                ("text2", k.text2, "secondary text"),
                ("text3", k.text3, "hints, disabled (non-essential)"),
                ("accent", k.accent, "primary action, selection"),
                ("on_accent", k.on_accent, "text on accent"),
                ("accent_soft", k.accent_soft, "selected row / chip fill"),
                ("focus", k.focus, "keyboard focus ring"),
                ("audio", k.audio, "audio jack and cable"),
                ("cv", k.cv, "control / modulation jack, cable, ring"),
                ("gate", k.gate, "gate and trigger jack, cable"),
                ("good", k.good, "ok state"),
                ("warn", k.warn, "warning"),
                ("bad", k.bad, "error, clip"),
                ("rack", k.rack, "rack background"),
                ("rail", k.rail, "rack rail"),
                ("hole", k.hole, "jack hole, rail slot"),
                ("disp_bg", k.disp_bg, "live display background"),
                ("disp_grid", k.disp_grid, "display graticule"),
                ("disp_trace", k.disp_trace, "display trace"),
                ("disp_text", k.disp_text, "display text"),
                ("knob", k.knob, "knob body"),
                ("knob_hi", k.knob_hi, "knob rim"),
                ("knob_ink", k.knob_ink, "knob pointer"),
            ];
            for (n, c, u) in rows {
                println!("| `{n}` | `{}` | {u} |", hex(c));
            }
            println!("\n### Module face families\n\n| section | base | ink | ink2 | ink/base contrast |\n|---|---|---|---|---|");
            for s in Sect::ALL {
                let f = k.sect(s);
                println!(
                    "| {} | `{}` | `{}` | `{}` | {:.1} |",
                    s.name(),
                    hex(f.base),
                    hex(f.ink),
                    hex(f.ink2),
                    contrast(f.ink, f.base)
                );
            }
            println!();
        }
        let k = tok(dir, false);
        println!(
            "## Type scale (IBM Plex, OFL-1.1)\n\n| step | size | face | role |\n|---|---|---|---|"
        );
        for t in k.type_scale() {
            let fam = match t.fam {
                "sans" => "Plex Sans Regular",
                "sans-med" => "Plex Sans Medium",
                "sans-semi" => "Plex Sans SemiBold",
                "cond-semi" => "Plex Sans Condensed SemiBold",
                "cond-med" => "Plex Sans Condensed Medium",
                "mono" => "Plex Mono Regular",
                _ => "Plex Mono Medium",
            };
            println!("| {} | {} px | {} | {} |", t.name, t.size, fam, t.role);
        }
        println!("\nNo text below 11 px at any zoom; faces keep an 11 px floor when the rack is zoomed out.\n");
        println!("## Spacing, radii, shadows, motion\n");
        println!(
            "Spacing scale (px): {}\n",
            SPACING
                .iter()
                .map(|s| format!("{s}"))
                .collect::<Vec<_>>()
                .join(" · ")
        );
        println!(
            "Radii: small {} · medium {} · large {} px\n",
            k.r_sm, k.r_md, k.r_lg
        );
        println!(
            "| shadow | offset y | blur | alpha (light) | alpha (dark) |\n|---|---|---|---|---|"
        );
        let kd = tok(dir, true);
        for (i, n) in ["sm (controls)", "md (cards, faces)", "lg (popovers)"]
            .iter()
            .enumerate()
        {
            println!(
                "| {n} | {} | {} | {} | {} |",
                k.sh[i].dy, k.sh[i].blur, k.sh[i].alpha, kd.sh[i].alpha
            );
        }
        let m = k.motion;
        println!("\n| motion | duration | easing |\n|---|---|---|");
        println!("| hover | {} ms | linear |", m.hover_ms);
        println!("| press | {} ms | linear |", m.press_ms);
        println!("| drawer / popover | {} ms | ease-out cubic |", m.drawer_ms);
        println!("| view switch | {} ms | ease-out cubic |", m.view_ms);
        println!(
            "| value glide (knob, display) | {} ms | ease-out cubic |",
            m.glide_ms
        );
        println!("| cue / beat pulse | {} ms | triangle |", m.pulse_ms);
        println!("| displays (scope, curves, rings) | every frame, display refresh | n/a |");
        println!("| cable signal beads | 90 px/s | linear |\n");
        println!("---\n");
    }
}
