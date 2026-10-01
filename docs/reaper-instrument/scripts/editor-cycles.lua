local dir=assert(os.getenv('KABL_D07_HOST'))
local f=assert(io.open(dir..'/editor-cycles.txt','w'))
local track=reaper.GetTrack(0,0)
reaper.SetEditCurPos(0,false,false);reaper.OnPlayButton()
local count=0;local next_time=reaper.time_precise()
local function cycle()
 if reaper.time_precise()<next_time then reaper.defer(cycle);return end
 local open=count%2==0
 reaper.TrackFX_Show(track,0,open and 3 or 2)
 f:write('transition ',count+1,' open=',tostring(open),' play=',reaper.GetPlayState(),' gain=',reaper.TrackFX_GetParamNormalized(track,0,0),'\n');f:flush()
 count=count+1;next_time=reaper.time_precise()+0.45
 if count<20 then reaper.defer(cycle) else
  reaper.Main_SaveProjectEx(0,dir..'/after-cycles.rpp',0)
  f:write('10 complete open/close cycles; final editor closed\n');f:close()
 end
end
reaper.defer(cycle)
