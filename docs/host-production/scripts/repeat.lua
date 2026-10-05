local dir=assert(os.getenv('KABL_D08_HOST'))
for n=1,3 do dofile(assert(os.getenv('KABL_D08_SCRIPTS'))..'/render.lua') end
for i=0,reaper.CountTracks(0)-1 do reaper.TrackFX_Delete(reaper.GetTrack(0,i),0) end
local f=assert(io.open(dir..'/repeat.done','w'));f:write('three completed renders; instance destruction flushed diagnostic telemetry\n');f:close()
