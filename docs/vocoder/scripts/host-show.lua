-- Opens the first FX of the last track in its own window and plays, so a screenshot shows the
-- plugin editor with live displays. Writes KABL_HOST_DIR/KABL_RENDER_NAME.done after 8 s of
-- playback and leaves REAPER open for the screenshot (host-run.py closes it).
local dir = assert(os.getenv('KABL_HOST_DIR'))
local name = assert(os.getenv('KABL_RENDER_NAME'))
local started = reaper.time_precise()
local shown, played = false, false
local function step()
  local t = reaper.time_precise() - started
  local tr = reaper.GetTrack(0, reaper.CountTracks(0) - 1)
  if t > 4 and not shown then reaper.TrackFX_Show(tr, 0, 3); shown = true end
  if t > 6 and not played then reaper.OnPlayButton(); played = true end
  if t > 14 then
    local f = assert(io.open(dir .. '/' .. name .. '.done', 'w')); f:write('shown\n'); f:close()
    return
  end
  reaper.defer(step)
end
reaper.defer(step)
