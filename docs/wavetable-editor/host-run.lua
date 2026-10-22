-- Command loop for a scripted REAPER run. The shell writes `cmd` in KABL_HOST_DIR:
--   show        open the first track's FX editor (floating)
--   hide        close it
--   save        save the project to KABL_HOST_DIR/saved.rpp
--   render N    render 0..8 s to KABL_HOST_DIR/N.wav
--   quit        close REAPER
local dir = assert(os.getenv('KABL_HOST_DIR'))
local started = reaper.time_precise()
local function done(name, text)
  local f = assert(io.open(dir .. '/' .. name, 'w')); f:write(text or '1'); f:close()
end
local function render(name)
  reaper.OnStopButton()
  reaper.TrackFX_Show(reaper.GetTrack(0, 0), 0, 2)
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
end
local function step()
  if reaper.time_precise() - started < 3 then reaper.defer(step) return end
  if not io.open(dir .. '/ready', 'r') then
    local tr = reaper.GetTrack(0, 0)
    local _, name = reaper.TrackFX_GetFXName(tr, 0, '')
    done('info.txt', 'REAPER=' .. reaper.GetAppVersion() .. ' fx=' .. name .. '\n')
    done('ready')
  end
  local c = io.open(dir .. '/cmd', 'r')
  if c then
    local line = c:read('*l') or ''
    c:close(); os.remove(dir .. '/cmd')
    local verb, arg = line:match('^(%S+)%s*(.*)$')
    if verb == 'show' then reaper.TrackFX_Show(reaper.GetTrack(0, 0), 0, 3); done('shown')
    elseif verb == 'hide' then reaper.TrackFX_Show(reaper.GetTrack(0, 0), 0, 2); done('hidden')
    elseif verb == 'save' then reaper.Main_SaveProjectEx(0, dir .. '/saved.rpp', 0); done('saved')
    elseif verb == 'render' then render(arg); done(arg .. '.done')
    elseif verb == 'quit' then reaper.Main_OnCommand(40004, 0) return end
  end
  reaper.defer(step)
end
reaper.defer(step)
