local dir=assert(os.getenv('KABL_D08_HOST'))
local started=reaper.time_precise()
reaper.Audio_Init()
local function check()
 if reaper.time_precise()-started<1 then reaper.defer(check);return end
 local f=assert(io.open(dir..'/recall-host.txt','w'))
 assert(reaper.CountTracks(0)==2)
 for i=0,1 do
  local tr=reaper.GetTrack(0,i)
  assert(reaper.TrackFX_GetCount(tr)==1)
  local ok,name=reaper.TrackFX_GetFXName(tr,0,'');assert(ok and name:find('kabl'))
  local gain=reaper.TrackFX_GetParamNormalized(tr,0,0)
  local host=reaper.TrackFX_GetParamNormalized(tr,0,17)
  assert(math.abs(gain-(i==0 and .45 or .6))<1e-6 and host==1)
  f:write('track=',i,' gain=',gain,' host=',host,' slot1=',reaper.TrackFX_GetParamNormalized(tr,0,1),'\n')
  reaper.TrackFX_Show(tr,0,2)
 end
 reaper.Main_SaveProjectEx(0,dir..'/recalled.rpp',0)
 f:write('fresh-process recall saved\n');f:close()
 local done=assert(io.open(dir..'/recall.done','w'));done:write('recall complete\n');done:close()
end
reaper.defer(check)
