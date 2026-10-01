local dir = assert(os.getenv('KABL_D07_HOST'))
local f = assert(io.open(dir..'/show-rack.txt','w'))
local track = reaper.GetTrack(0,0)
f:write('tracks=',reaper.CountTracks(0),'\n')
for i=0,reaper.TrackFX_GetCount(track)-1 do
 local ok,name = reaper.TrackFX_GetFXName(track,i)
 f:write(i,' ',name,'\n')
 reaper.TrackFX_Show(track,i,3)
end
f:close()
