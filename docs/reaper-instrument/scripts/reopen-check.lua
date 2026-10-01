local dir=assert(os.getenv('KABL_D07_HOST'))
local f=assert(io.open(dir..'/reopen-check.txt','w'))
f:write('REAPER ',reaper.GetAppVersion(),' tracks=',reaper.CountTracks(0),'\n')
for index=0,reaper.CountTracks(0)-1 do
 local track=reaper.GetTrack(0,index)
 local ok,name=reaper.TrackFX_GetFXName(track,0)
 f:write('track ',index,' fx=',name,' gain=',reaper.TrackFX_GetParamNormalized(track,0,0),'\n')
 reaper.TrackFX_Show(track,0,2)
end
reaper.Main_SaveProjectEx(0,dir..'/reopened.rpp',0)
f:close()
reaper.SetEditCurPos(0,false,false)
reaper.OnPlayButton()
