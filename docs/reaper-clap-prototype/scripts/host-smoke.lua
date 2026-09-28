-- Run with REAPER's fresh -cfgfile profile and KABL_PROOF_DIR set to a throwaway directory.
-- KABL_PROOF_MODE=create creates a two-instance MIDI project; reopen inspects saved state.
local dir = assert(os.getenv('KABL_PROOF_DIR'), 'set KABL_PROOF_DIR')
local mode = assert(os.getenv('KABL_PROOF_MODE'), 'set KABL_PROOF_MODE')
local log_file = assert(io.open(dir .. '/' .. mode .. '.log', 'w'))
local function log(message)
  log_file:write(tostring(message), '\n')
  log_file:flush()
end
log('REAPER ' .. reaper.GetAppVersion())

if mode == 'create' then
  for i = 0, 1 do
    reaper.InsertTrackAtIndex(i, true)
    local track = reaper.GetTrack(0, i)
    reaper.GetSetMediaTrackInfo_String(track, 'P_NAME', 'Kabl ' .. (i + 1), true)
    local fx = reaper.TrackFX_AddByName(track, 'CLAP: kabl REAPER proof', false, -1)
    log('track ' .. i .. ' CLAP insert ' .. fx)
    assert(fx >= 0, 'CLAP scan/insert failed')
    local item = reaper.CreateNewMIDIItemInProj(track, 0, 2, false)
    local take = reaper.GetActiveTake(item)
    reaper.MIDI_InsertNote(take, false, false, 0, 960, 0, 60 + i * 7, 110, false)
    reaper.MIDI_Sort(take)
  end
  reaper.GetSetProjectInfo_String(0, 'RENDER_FILE', dir, true)
  reaper.GetSetProjectInfo_String(0, 'RENDER_PATTERN', 'proof-render', true)
  reaper.GetSetProjectInfo(0, 'RENDER_BOUNDSFLAG', 1, true)
  reaper.GetSetProjectInfo(0, 'RENDER_SRATE', 48000, true)
  reaper.Main_SaveProjectEx(0, dir .. '/proof.rpp', 0)
  log('saved proof.rpp with two MIDI tracks')
elseif mode == 'reopen' then
  log('tracks ' .. reaper.CountTracks(0))
  for i = 0, reaper.CountTracks(0) - 1 do
    local track = reaper.GetTrack(0, i)
    local count = reaper.TrackFX_GetCount(track)
    local _, name = reaper.TrackFX_GetFXName(track, 0, '')
    local value, minimum, maximum = reaper.TrackFX_GetParam(track, 0, 0)
    log('track ' .. i .. ' FX ' .. count .. ' name ' .. name .. ' cutoff ' .. value .. ' range ' .. minimum .. '..' .. maximum)
  end
  if reaper.CountTracks(0) > 0 then
    local track = reaper.GetTrack(0, 0)
    log('set track 0 cutoff to normalized 0.25: ' .. tostring(reaper.TrackFX_SetParamNormalized(track, 0, 0, 0.25)))
    local start = reaper.time_precise()
    local function later()
      if reaper.time_precise() - start < 1.0 then reaper.defer(later); return end
      for i = 0, reaper.CountTracks(0) - 1 do
        local value = reaper.TrackFX_GetParamNormalized(reaper.GetTrack(0, i), 0, 0)
        log('after 1s track ' .. i .. ' cutoff normalized ' .. value)
      end
      reaper.Main_SaveProjectEx(0, dir .. '/proof-recalled.rpp', 0)
      log('saved proof-recalled.rpp')
      log_file:close()
    end
    reaper.defer(later)
  end
else
  error('unknown KABL_PROOF_MODE: ' .. mode)
end
if mode == 'create' then log_file:close() end
