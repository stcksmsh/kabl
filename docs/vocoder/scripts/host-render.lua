-- Renders the open project (0..10 s, 48 kHz stereo float WAV) to KABL_HOST_DIR/KABL_RENDER_NAME.wav,
-- writes KABL_HOST_DIR/KABL_RENDER_NAME.done, and closes REAPER.
local dir = assert(os.getenv('KABL_HOST_DIR'))
local name = assert(os.getenv('KABL_RENDER_NAME'))
local started = reaper.time_precise()
local function done(text)
  local f = assert(io.open(dir .. '/' .. name .. '.done', 'w')); f:write(text or '1'); f:close()
end
local function step()
  if reaper.time_precise() - started < 4 then reaper.defer(step) return end
  local tr = reaper.GetTrack(0, reaper.CountTracks(0) - 1)
  local _, fx = reaper.TrackFX_GetFXName(tr, 0, '')
  local nin = reaper.TrackFX_GetNumInputs and 'n/a' or 'n/a'
  reaper.GetSetProjectInfo_String(0, 'RENDER_FILE', dir, true)
  reaper.GetSetProjectInfo_String(0, 'RENDER_PATTERN', name, true)
  reaper.GetSetProjectInfo(0, 'RENDER_BOUNDSFLAG', 0, true)
  reaper.GetSetProjectInfo(0, 'RENDER_STARTPOS', 0, true)
  reaper.GetSetProjectInfo(0, 'RENDER_ENDPOS', 10, true)
  reaper.GetSetProjectInfo(0, 'RENDER_SRATE', 48000, true)
  reaper.GetSetProjectInfo(0, 'RENDER_CHANNELS', 2, true)
  reaper.GetSetProjectInfo(0, 'RENDER_TAILFLAG', 0, true)
  reaper.GetSetProjectInfo(0, 'RENDER_SETTINGS', 0, true)
  reaper.GetSetProjectInfo_String(0, 'RENDER_FORMAT', 'ZXZhdyAAAA==', true)
  reaper.Main_OnCommand(42230, 0)
  local _, pins_in, pins_out = reaper.TrackFX_GetIOSize(tr, 0)
  done('fx=' .. fx .. ' inputs=' .. tostring(pins_in) .. ' outputs=' .. tostring(pins_out) .. ' REAPER=' .. reaper.GetAppVersion() .. '\n')
  reaper.Main_OnCommand(40004, 0)
end
reaper.defer(step)
