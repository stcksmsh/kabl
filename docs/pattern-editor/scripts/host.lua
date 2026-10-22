-- Opens the plugin's floating editor on track 1 and plays the project, so the pattern editor can
-- be driven with real input and captured while the host runs audio. Written to be driven by
-- host.py; REAPER stays up until host.py ends it.
local started = reaper.time_precise()
local shown = false
local function step()
  if reaper.time_precise() - started < 3 then reaper.defer(step) return end
  if not shown then
    shown = true
    local tr = reaper.GetTrack(0, 0)
    reaper.TrackFX_Show(tr, 0, 3)
    reaper.OnPlayButton()
  end
  reaper.defer(step)
end
reaper.defer(step)
