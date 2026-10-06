-- Actual REAPER private-profile save, embedded recall and native automation readback.
local dir=assert(os.getenv('KABL_D10_HOST'))
for i=0,1 do
 local tr=assert(reaper.GetTrack(0,i));assert(reaper.TrackFX_GetCount(tr)==1)
 reaper.SetMediaTrackInfo_Value(tr,'I_PERFFLAGS',3)
 reaper.TrackFX_Show(tr,0,0);reaper.TrackFX_Show(tr,0,2)
 local item=reaper.GetTrackMediaItem(tr,0);if item then reaper.SetMediaItemInfo_Value(item,'D_LENGTH',36) end
end
reaper.Audio_Init();reaper.OnStopButton();reaper.GetSetRepeat(0)
local start=reaper.time_precise();local playing=false;local sample=1
local times={2,6,13};local f=assert(io.open(dir..'/native-replay.txt','w'))
f:write('REAPER=',reaper.GetAppVersion(),'\n')
local function step()
 if not playing and reaper.time_precise()-start>1 then
  reaper.Main_SaveProjectEx(0,dir..'/recalled.rpp',0)
  reaper.SetEditCurPos(0,false,false);reaper.OnPlayButton();playing=true
 end
 local pos=reaper.GetPlayPosition()
 if playing and sample<=#times and pos>times[sample] then
  for i=0,1 do
   local tr=reaper.GetTrack(0,i)
   assert(reaper.TrackFX_GetFloatingWindow(tr,0)==nil and reaper.TrackFX_GetChainVisible(tr)==-1)
   for slot=1,(i==0 and 1 or 2) do
    local env=assert(reaper.GetFXEnvelope(tr,0,slot,false));local ok,expected=reaper.Envelope_Evaluate(env,pos,48000,1);assert(ok)
    local actual=reaper.TrackFX_GetParamNormalized(tr,0,slot)
    f:write('track=',i,' slot=',slot,' time=',pos,' expected=',expected,' actual=',actual,'\n');f:flush()
    assert(math.abs(actual-expected)<.025,'Native automation diverged')
   end
  end
  sample=sample+1
 end
 if sample>#times then
  reaper.OnStopButton();f:write('closed-editor automation passed\n');f:close()
  local done=assert(io.open(dir..'/done','w'));done:write('Recall + native replay complete\n');done:close();return
 end
 reaper.defer(step)
end
reaper.defer(step)
