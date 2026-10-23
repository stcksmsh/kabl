-- Opens the plugin's floating editor in REAPER and keeps the project playing so cues can be sent.
local dir=assert(os.getenv('KABL_SE_HOST'))
local started=reaper.time_precise()
local shown=false
local function step()
 local t=reaper.time_precise()-started
 if t>2 and not shown then
  shown=true
  reaper.TrackFX_Show(reaper.GetTrack(0,0),0,3)
  reaper.OnPlayButton()
  local f=assert(io.open(dir..'/editor-open','w'));f:write('ok');f:close()
 end
 if t<90 then reaper.defer(step) end
end
reaper.defer(step)
