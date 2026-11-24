//! Start-up: arguments, logging, the first document, audio and MIDI, the window.

use super::*;

use kabl_ui::{record, PatchEditor, UiState};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

/// `kabl-ui [--patch <file.kabl>] [--size <W>x<H>]`. Without `--patch`, starts from
/// `default_patch()`. Default window 1440×900; 1280×800 is the supported minimum.
pub(crate) fn run() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1).cloned())
    };
    use kabl_standalone::applog;
    let (level, bad_level) = applog::level_from(&args);
    let mut log = applog::init("kabl-ui", level, applog::log_dir());
    log::info!(
        target: "app",
        "start kabl-ui {} level={level} log={}",
        applog::build_id(),
        log.as_ref().map_or("unavailable".into(), |l| l.path.display().to_string())
    );
    if let Some(w) = bad_level {
        log::warn!(target: "app", "unknown log level {w:?}: using info");
    }
    let mut ui_state = UiState::default();
    ui_state.use_user_prefs();
    let remembered = kabl_ui::first_run::devices::take_over(&mut ui_state);
    // D6: the computer keyboard plays notes in the standalone; the key strip waits to be opened.
    ui_state.keys.computer = true;
    let library = kabl_ui::browser::open_library();
    log::info!(
        target: "library",
        "factory sounds: {}; your sounds: {}",
        library
            .factory_dir
            .as_ref()
            .map_or("not found".into(), |d| d.display().to_string()),
        library.sounds_dir().display()
    );
    for note in &library.notes {
        log::warn!(target: "library", "{note}");
    }
    let named = match flag("--patch") {
        Some(dir) => match open_document(dir, &library, &mut ui_state) {
            Some(e) => Some(e),
            None => {
                if let Some(l) = log.as_mut() {
                    l.shutdown();
                }
                std::process::exit(1);
            }
        },
        None => None,
    };
    ui_state.library = Some(library);
    // No patch named: the start patch (with lesson 00 over it on a fresh profile).
    let editor = named.unwrap_or_else(|| kabl_ui::first_run::open(&mut ui_state));
    ui_state.browser.async_io = true;
    ui_state.browser.io_delay_ms = std::env::var("KABL_BROWSER_IO_DELAY_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    if args.iter().any(|a| a == "--browser") {
        ui_state.browser_open = true;
    }
    let size = flag("--size")
        .and_then(|s| {
            let (w, h) = s.split_once('x')?;
            Some([w.parse().ok()?, h.parse().ok()?])
        })
        .unwrap_or([1440.0, 900.0]);
    let (notes_tx, notes_rx) = rtrb::RingBuffer::<Note>::new(1024);
    let (mut midi, cc_rx) = Midi::new(notes_tx);
    // `--midi` wins for this launch and is not remembered.
    let midi_missing = match flag("--midi") {
        Some(f) => {
            midi.connect_default(Some(&f));
            None
        }
        None => midi.connect_remembered(&remembered.midi),
    };
    if let Some(n) = midi_missing {
        log::info!(target: "midi", "remembered input \"{n}\" not found: connecting when it appears");
    }
    let outputs = output_names();
    let (device, output_missing) = remembered_output(&remembered.output, &outputs);
    if let Some(n) = &output_missing {
        log::warn!(target: "audio", "remembered output \"{n}\" not found: using the system default");
        ui_state.last_message = Some(format!(
            "Output \"{n}\" not found: playing on the system default"
        ));
    }
    let num = |name: &str| flag(name).and_then(|v| v.parse::<u32>().ok());
    let peaks = Arc::new(record::PeakTap::default());
    let request = AudioRequest {
        rate: num("--rate"),
        frames: num("--frames"),
        realtime: !args.iter().any(|a| a == "--no-rt"),
        device,
        input: args.iter().any(|a| a == "--input"),
    };
    let (mut audio, mut delivery) = AudioHost::start(
        editor.state(),
        notes_rx,
        midi.keys.clone(),
        &request,
        false,
        peaks.clone(),
    );
    delivery.async_compile = true;
    delivery.compile_delay_ms = std::env::var("KABL_COMPILE_DELAY_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let phase = if audio._stream.is_some() {
        AudioPhase::Starting
    } else {
        AudioPhase::Unavailable
    };
    ui_state.inspect.rebuilt(1, None, editor.state(), 0.0);
    ui_state.recorder = audio.recorder.take();
    ui_state.sample_rate = audio.sample_rate;
    ui_state.browser.clips.attach(audio.preview.take());
    if let (Some(rec), Some(dir)) = (ui_state.recorder.as_mut(), flag("--record-dir")) {
        rec.dir = dir;
    }
    if args.iter().any(|a| a == "--perform") {
        ui_state.perform_open = true;
    }
    let record_dir = ui_state
        .recorder
        .as_ref()
        .map_or_else(|| "Recordings".into(), |r| r.dir.clone());

    let options = window_options(size);
    let result = eframe::run_native(
        "kabl",
        options,
        Box::new(|cc| {
            kabl_ui::theme::install_fonts(&cc.egui_ctx);
            let compile_notice = CompileNotice::new(&delivery);
            let core = Arc::new(Mutex::new(Core {
                editor,
                ui: ui_state,
                delivery,
                latency: std::env::var_os("KABL_LATENCY_FILE").map(|_| Latency::default()),
                progress: audio
                    .timing
                    .as_ref()
                    .map(|t| Progress::new(t.clone(), audio.gate.clone())),
                record_finish: None,
                record_dir: record_dir.clone(),
                record_last: None,
                cc_cutoff: f64::NEG_INFINITY,
                stale_cc: 0,
            }));
            let c = core.clone();
            let control_stop = Arc::new(AtomicBool::new(false));
            let stop = control_stop.clone();
            let pump = std::thread::Builder::new()
                .name("kabl-control".into())
                .spawn(move || pump(c, cc_rx, stop))
                .expect("spawn the control thread");
            {
                let mut sink = midi.sink.lock().unwrap();
                sink.ctx = Some(cc.egui_ctx.clone());
                sink.pump = Some(pump.thread().clone());
            }
            Ok(Box::new(App {
                core,
                compile_notice,
                latency_at: std::time::Instant::now(),
                audio,
                midi,
                hits_file: std::env::var("KABL_HITS_FILE").ok(),
                peaks,
                meter_at: std::time::Instant::now(),
                watchdog: (0, std::time::Instant::now()),
                hits_written: String::new(),
                stats_file: std::env::var("KABL_STATS_FILE")
                    .ok()
                    .map(|f| (f, std::time::Instant::now())),
                log: log.as_ref().map(|l| l.view()),
                health: Health::new(),
                request,
                outputs,
                output_scanned: std::time::Instant::now(),
                phase,
                retry: None,
                session: 1,
                pending_notes: None,
                pending_recorder: None,
                record_finish: None,
                record_last: None,
                control_stop,
                control_thread: Some(pump),
            }))
        }),
    );
    log::info!(target: "app", "shutdown result={}", if result.is_ok() { "ok" } else { "error" });
    if let Some(l) = log.as_mut() {
        l.shutdown();
    }
    result
}

/// The `--patch` file (keeping its library identity when it is a library sound). `None` when it
/// fails to load (logged).
fn open_document(
    dir: String,
    library: &kabl_ui::library::Library,
    ui_state: &mut UiState,
) -> Option<PatchEditor> {
    match kabl_core::format::load_kabl_full(std::path::Path::new(&dir)) {
        Ok((log, _, code)) => {
            // Named on the command line, so its embedded code is registered without the
            // trust notice the browser shows.
            for w in kabl_standalone::code_load::register_for_cli(&code, log.state()) {
                log::warn!(target: "app", "{w}");
            }
            let editor = PatchEditor::from_log(log);
            // A library sound opened by path keeps its library identity (factory
            // protection, name); anything else is a plain folder.
            let canonical = std::path::Path::new(&dir).canonicalize().ok();
            let doc = match library
                .entries
                .iter()
                .find(|e| e.dir.canonicalize().ok() == canonical)
            {
                Some(e) => kabl_ui::browser::Doc {
                    name: e.meta.name.clone(),
                    origin: kabl_ui::browser::DocOrigin::Library(e.id.clone()),
                    meta: e.meta.clone(),
                    saved: editor.state().clone(),
                },
                None => {
                    let name = std::path::Path::new(&dir)
                        .file_name()
                        .map_or(dir.clone(), |n| n.to_string_lossy().to_string());
                    kabl_ui::browser::Doc::new(
                        &name,
                        kabl_ui::browser::DocOrigin::Folder(dir.clone()),
                        editor.state(),
                    )
                }
            };
            ui_state.browser.folder = dir;
            ui_state.doc = Some(doc);
            Some(editor)
        }
        Err(err) => {
            log::error!(target: "doc", "failed to load patch from {dir}: {err:?}");
            None
        }
    }
}

/// The window: title, icon, size and the supported minimum.
fn window_options(size: [f32; 2]) -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("kabl")
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!(
                    "../../../../packaging/linux/logo/hicolor/256x256/apps/kabl.png"
                ))
                .expect("bundled app icon"),
            )
            .with_inner_size(size)
            .with_min_inner_size([1280.0, 800.0]),
        ..Default::default()
    }
}
