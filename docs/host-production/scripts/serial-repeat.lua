local dir=assert(os.getenv('KABL_D08_HOST'))
reaper.Audio_Init()
local started=reaper.time_precise()
local function render_ready()
 if reaper.time_precise()-started<1 then reaper.defer(render_ready);return end
local log=assert(io.open(dir..'/serial-host-info.txt','w'))
local project,path=reaper.EnumProjects(-1,'');log:write('project=',path,'\n')
for n=0,reaper.CountTempoTimeSigMarkers(0)-1 do
 local ok,t,m,b,bpm,num,denom,linear=reaper.GetTempoTimeSigMarker(0,n)
 log:write('tempo ',n,' ',t,' ',bpm,'\n')
end
for i=0,reaper.CountTracks(0)-1 do reaper.SetMediaTrackInfo_Value(reaper.GetTrack(0,i),'I_PERFFLAGS',3) end
reaper.Audio_Quit()
log:write('audio=',reaper.Audio_IsRunning(),'\n');log:close()
for n=1,3 do dofile(assert(os.getenv('KABL_D08_SCRIPTS'))..'/render.lua') end
local f=assert(io.open(dir..'/serial.done','w'));f:write('three completed device-closed serial renders\n');f:close()

end
reaper.defer(render_ready)
