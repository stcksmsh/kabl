#!/usr/bin/env bash
# The D06 owner test bundle, outside the checkout:
#   OUT=/path/kabl-d06-test docs/midi-timing/scripts/make-bundle.sh
# Contents: the relocatable package (packaging/linux/package.sh), the demo performances and a
# player to replay them live into the app, the offline listening WAVs, the walkthrough and
# START-HERE.txt. Nothing is read from the source checkout at run time.
set -euo pipefail
OUT=${OUT:?}
rm -rf "$OUT"
mkdir -p "$OUT/listening" "$OUT/demo"
pkg=$(OUT="$OUT/pkg" bash packaging/linux/package.sh | tail -1)
tar -C "$OUT" -xzf "$pkg"
rm -rf "$OUT/pkg"
app=$(ls -d "$OUT"/kabl-*-linux-*)
mv "$app" "$OUT/kabl"
cargo build --release -p kabl-ui --example midi_player -p kabl-engine --example render_midi
cp target/release/examples/midi_player "$OUT/kabl/bin/"
python3 docs/midi-timing/scripts/make_demos.py docs/midi-timing/scripts
cp docs/midi-timing/scripts/{lead,keys,timing}.events docs/midi-timing/scripts/play_events.py "$OUT/demo/"
r=target/release/examples/render_midi
$r patches/expressive/lead docs/midi-timing/scripts/lead.events "$OUT/listening/lead.wav" --frames irregular
$r patches/expressive/split docs/midi-timing/scripts/keys.events "$OUT/listening/keys.wav" --frames irregular
$r patches/expressive/split docs/midi-timing/scripts/timing.events "$OUT/listening/timing.wav" --frames irregular
$r patches/expressive/split docs/midi-timing/scripts/timing.events \
  "$OUT/listening/timing-pre-d06-emulation.wav" --policy pre-d06
cp docs/midi-timing/walkthrough.mp4 docs/midi-timing/CHECKLIST.md "$OUT/"
cat > "$OUT/play-demo.sh" <<'SH'
#!/usr/bin/env bash
# Replays a demo performance live into the running app through a virtual MIDI port
# (scripted MIDI, not your controller):  ./play-demo.sh lead|keys|timing
# Start the app first:  ./kabl/bin/kabl-ui --patch kabl/share/kabl/patches/expressive/lead --midi kabl-player
set -euo pipefail
cd "$(dirname "$0")"
python3 demo/play_events.py "demo/${1:?lead|keys|timing}.events" 3 | ./kabl/bin/midi_player
SH
chmod +x "$OUT/play-demo.sh"
cat > "$OUT/START-HERE.txt" <<TXT
kabl D06 test bundle — expressive MIDI and event timing
Built from $(git rev-parse --short HEAD) (branch codex/d06-expressive-midi). Owner review pending.

1. Play with your controller (48 kHz, 256 frames):
     ./kabl/bin/kabl-ui --patch kabl/share/kabl/patches/expressive/lead --perform --rate 48000 --frames 256
   Pick your controller under Perform > MIDI in. Other demo: patches/expressive/split
   (channel 1 keys, channel 2 bass). Or open either from Sounds: "Expressive Lead",
   "Keys and Bass by Channel".
2. Listen (offline renders through the production timing adapter, 48 kHz float WAV):
     listening/lead.wav, keys.wav, timing.wav, timing-pre-d06-emulation.wav
     e.g.  pw-play listening/lead.wav   (or aplay / any player)
3. Watch walkthrough.mp4 (real app, scripted virtual MIDI, sound = the app's own recording).
4. Optional: hear the demo performances live in the app without a controller:
     start the app with  --midi kabl-player  after running  ./play-demo.sh lead  in another
     terminal (start play-demo first so the port exists; it waits 3 s).
5. Tick CHECKLIST.md and send the results.

Your sounds and settings live in ~/.local/share/kabl as usual; set KABL_USER_DIR=/tmp/kabl-test
to keep this test separate. Nothing here needs the source checkout.
TXT
ls -R "$OUT" | head -40
