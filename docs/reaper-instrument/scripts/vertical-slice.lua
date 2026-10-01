local dir = assert(os.getenv('KABL_D07_HOST'))
local f = assert(io.open(dir .. '/vertical-slice.txt', 'w'))
local function log(s) f:write(tostring(s),'\n'); f:flush() end
log('REAPER ' .. reaper.GetAppVersion())
reaper.InsertTrackAtIndex(0, true)
local track = reaper.GetTrack(0,0)
local fx = reaper.TrackFX_AddByName(track, 'CLAP: kabl', false, -1)
log('insert=' .. fx)
assert(fx >= 0, 'production CLAP scan failed')
reaper.TrackFX_Show(track, fx, 3)
local item = reaper.CreateNewMIDIItemInProj(track, 0, 8, false)
local take = reaper.GetActiveTake(item)
for i=0,7 do reaper.MIDI_InsertNote(take,false,false,i*960,i*960+720,0,60+i%3*4,100,false) end
reaper.MIDI_Sort(take)
local gain_index = nil
for i=0,reaper.TrackFX_GetNumParams(track,fx)-1 do
 local ok,name = reaper.TrackFX_GetParamName(track,fx,i)
 log('parameter '..i..' '..tostring(name)..' '..reaper.TrackFX_GetParamNormalized(track,fx,i))
 if name == 'Output gain' then gain_index = i end
end
assert(gain_index, 'stable host output gain missing')
log('gain set='..tostring(reaper.TrackFX_SetParamNormalized(track,fx,gain_index,0.4)))
reaper.OnPlayButton()
local start = reaper.time_precise()
local function check()
 if reaper.time_precise()-start < 2 then reaper.defer(check); return end
 log('gain=' .. reaper.TrackFX_GetParamNormalized(track,fx,gain_index))
 reaper.Main_SaveProjectEx(0,dir..'/vertical-slice.rpp',0)
 reaper.OnPlayButton()
 log('play=' .. reaper.GetPlayState())
 f:close()
end
reaper.defer(check)
