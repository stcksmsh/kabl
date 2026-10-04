local dir=assert(os.getenv('KABL_D08_HOST'))
reaper.Audio_Init();reaper.OnStopButton()
for i=0,1 do reaper.TrackFX_Show(reaper.GetTrack(0,i),0,0);reaper.TrackFX_Show(reaper.GetTrack(0,i),0,2) end
local tr=reaper.GetTrack(0,0)
reaper.SetMediaTrackInfo_Value(tr,'I_AUTOMODE',1)
local start=reaper.time_precise();local phase=0
local f=assert(io.open(dir..'/replay-host.txt','w'))
local function step()
 if phase==0 and reaper.time_precise()-start>1 then
  for i=0,1 do reaper.TrackFX_Show(reaper.GetTrack(0,i),0,0);reaper.TrackFX_Show(reaper.GetTrack(0,i),0,2) end
  reaper.SetEditCurPos(0,false,false);reaper.OnPlayButton();phase=1
 elseif phase==1 and reaper.GetPlayPosition()>2 then
  local value=reaper.TrackFX_GetParamNormalized(tr,0,1)
  assert(math.abs(value-.4)<.02);f:write('read at ',reaper.GetPlayPosition(),' = ',value,'\n');phase=2
 elseif phase==2 and reaper.GetPlayPosition()>5.5 then
  local value=reaper.TrackFX_GetParamNormalized(tr,0,1)
  assert(math.abs(value-.75)<.02);f:write('read at ',reaper.GetPlayPosition(),' = ',value,'\n');phase=3
 elseif phase==3 and reaper.GetPlayPosition()>7 then
  for i=0,1 do assert(reaper.TrackFX_GetFloatingWindow(reaper.GetTrack(0,i),0)==nil and reaper.TrackFX_GetChainVisible(reaper.GetTrack(0,i))==-1) end
  reaper.OnStopButton();f:write('editors closed; real saved automation replay verified\n');f:close()
  local done=assert(io.open(dir..'/replay.done','w'));done:write('automation replay complete\n');done:close();return
 end
 reaper.defer(step)
end
reaper.defer(step)
