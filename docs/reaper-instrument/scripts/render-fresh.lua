local dir=assert(os.getenv('KABL_D07_HOST'))
assert(reaper.CountTracks(0)==2,'expected unmodified two-instance scratch project')
reaper.OnStopButton()
for index=0,1 do
 local track=reaper.GetTrack(0,index)
 reaper.TrackFX_Show(track,0,2)
 reaper.TrackFX_SetOffline(track,0,true)
 reaper.TrackFX_SetOffline(track,0,false)
end
-- Fresh host instantiation clears free phase, held keys, delay/reverb buffers and Timeline.
-- Do not edit this project or inject live MIDI until render-fresh.done exists.
reaper.GetSetProjectInfo_String(0,'RENDER_FILE',dir,true)
reaper.GetSetProjectInfo_String(0,'RENDER_PATTERN','d07-fresh',true)
local start=reaper.time_precise()
reaper.Main_OnCommand(42230,0)
local f=assert(io.open(dir..'/render-fresh.done','w'))
f:write('seconds=',reaper.time_precise()-start,'\n');f:close()
