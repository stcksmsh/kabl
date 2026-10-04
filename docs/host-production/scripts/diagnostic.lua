dofile(assert(os.getenv('KABL_D08_HOST'))..'/../../docs/host-production/scripts/render.lua')
for i=0,reaper.CountTracks(0)-1 do reaper.TrackFX_Delete(reaper.GetTrack(0,i),0) end
local f=assert(io.open(os.getenv('KABL_D08_HOST')..'/diagnostic.done','w'));f:write('completed render and destroyed proxy instances\n');f:close()
