-- Fresh-process playback of the unchanged arrangement envelopes with both editors closed.
local dir=assert(os.getenv('KABL_D08_HOST'))
-- Use the same declared serial/non-anticipative profile as accepted D08 renders.
for i=0,1 do
 reaper.SetMediaTrackInfo_Value(reaper.GetTrack(0,i),'I_PERFFLAGS',3)
 local item=reaper.GetTrackMediaItem(reaper.GetTrack(0,i),0)
 if item then reaper.SetMediaItemInfo_Value(item,'D_LENGTH',36) end
end
reaper.GetSetRepeat(0) -- Reach the saved switch at 24 seconds instead of repeating the song loop.
reaper.Audio_Init();reaper.OnStopButton()
for i=0,1 do reaper.TrackFX_Show(reaper.GetTrack(0,i),0,0);reaper.TrackFX_Show(reaper.GetTrack(0,i),0,2) end
local start=reaper.time_precise();local playing=false;local sample=1
local times={2,6,13}
local f=assert(io.open(dir..'/d09-replay-host.txt','w'))
local function step()
 if not playing and reaper.time_precise()-start>1 then
  reaper.SetEditCurPos(0,false,false);reaper.OnPlayButton();playing=true
 end
 local position=reaper.GetPlayPosition()
 if playing and sample<=#times and position>times[sample] then
  for i=0,1 do
   local tr=reaper.GetTrack(0,i)
   assert(reaper.TrackFX_GetFloatingWindow(tr,0)==nil and reaper.TrackFX_GetChainVisible(tr)==-1)
   for slot=1,(i==0 and 1 or 2) do
    local envelope=assert(reaper.GetFXEnvelope(tr,0,slot,false))
    local ok,expected=reaper.Envelope_Evaluate(envelope,position,48000,1);assert(ok)
    local value=reaper.TrackFX_GetParamNormalized(tr,0,slot)
    f:write('track=',i,' slot=',slot,' time=',position,' expected=',expected,' actual=',value,'\n');f:flush()
    assert(math.abs(value-expected)<.025,'Native automation diverged from saved envelope')
   end
  end
  sample=sample+1
 end
 if sample>#times then
  reaper.OnStopButton();f:write('Both editors closed; continuous controls and Host sequence mode verified. Free transition beyond project end was not accepted as new evidence.\n');f:close()
  local done=assert(io.open(dir..'/d09-replay.done','w'));done:write('D09 native automation replay complete\n');done:close();return
 end
 reaper.defer(step)
end
reaper.defer(step)
