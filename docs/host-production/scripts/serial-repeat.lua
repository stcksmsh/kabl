local dir=assert(os.getenv('KABL_D08_HOST'))
local log=assert(io.open(dir..'/serial-host-info.txt','w'))
local project,path=reaper.EnumProjects(-1,'');log:write('project=',path,'\n')
for n=0,reaper.CountTempoTimeSigMarkers(0)-1 do
 local ok,t,m,b,bpm,num,denom,linear=reaper.GetTempoTimeSigMarker(0,n)
 log:write('tempo ',n,' ',t,' ',bpm,'\n')
end
for i=0,reaper.CountTracks(0)-1 do reaper.SetMediaTrackInfo_Value(reaper.GetTrack(0,i),'I_PERFFLAGS',3) end
reaper.Audio_Quit()
log:write('audio=',reaper.Audio_IsRunning(),'\n');log:close()
for n=1,3 do dofile(dir..'/../../docs/host-production/scripts/render.lua') end
for i=0,reaper.CountTracks(0)-1 do reaper.TrackFX_Delete(reaper.GetTrack(0,i),0) end
local f=assert(io.open(dir..'/serial.done','w'));f:write('three completed device-closed serial renders\n');f:close()
