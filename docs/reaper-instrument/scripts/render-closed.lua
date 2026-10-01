local dir=assert(os.getenv('KABL_D07_HOST'))
reaper.OnStopButton()
for index=0,reaper.CountTracks(0)-1 do reaper.TrackFX_Show(reaper.GetTrack(0,index),0,2) end
reaper.GetSetProjectInfo_String(0,'RENDER_FILE',dir,true)
reaper.GetSetProjectInfo_String(0,'RENDER_PATTERN','d07-musical',true)
reaper.Main_OnCommand(42230,0)
local f=assert(io.open(dir..'/render-closed.txt','w'))
local ok,stats=reaper.GetSetProjectInfo_String(0,'RENDER_STATS','',false)
f:write('editors closed; offline REAPER render\n',tostring(stats),'\n');f:close()
