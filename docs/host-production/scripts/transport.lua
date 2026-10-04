local dir=assert(os.getenv('KABL_D08_HOST'))
local log=assert(io.open(dir..'/transport-host.txt','w'))
local function line(s) log:write(tostring(s),'\n');log:flush() end
reaper.OnStopButton()
for i=0,1 do reaper.TrackFX_Show(reaper.GetTrack(0,i),0,2) end
reaper.Audio_Init()
line('tracks='..reaper.CountTracks(0)..' audio='..reaper.Audio_IsRunning())
for _,key in ipairs({'SRATE','BSIZE','MODE'}) do local ok,v=reaper.GetAudioDeviceInfo(key);line(key..'='..tostring(ok)..' '..v) end
reaper.SetEditCurPos(0,false,false)
local start=reaper.time_precise();local phase=-1
local function step()
 local elapsed=reaper.time_precise()-start
 if phase==-1 and elapsed>.5 then
  reaper.OnPlayButton();line('play='..reaper.GetPlayState()..' audio='..reaper.Audio_IsRunning());phase=0
 elseif phase==0 and elapsed>1.5 then
  reaper.SetCurrentBPM(0,150,true);line('tempo set=150 time='..reaper.GetPlayPosition());assert(reaper.GetPlayPosition()>.5,'Playback did not advance');phase=1
 elseif phase==1 and elapsed>2 then
  reaper.SetEditCurPos(8,false,true);line('seek=8');phase=2
 elseif phase==2 and elapsed>3 then
  reaper.GetSet_LoopTimeRange(true,true,8,9,false);reaper.GetSetRepeat(1);line('loop=8..9');phase=3
 elseif phase==3 and elapsed>5 then
  line('loop position='..reaper.GetPlayPosition());reaper.OnStopButton();line('stop='..reaper.GetPlayState());phase=4
 elseif phase==4 and elapsed>6 then
  reaper.GetSetRepeat(0);reaper.TrackFX_SetParamNormalized(reaper.GetTrack(0,1),0,17,0)
  line('Free='..reaper.TrackFX_GetParamNormalized(reaper.GetTrack(0,1),0,17));phase=5
 elseif phase==5 and elapsed>7 then
  local free=reaper.TrackFX_GetParamNormalized(reaper.GetTrack(0,1),0,17)
  line('Free delayed='..free);assert(free==0,'Free parameter did not settle')
  for i=0,1 do reaper.TrackFX_Delete(reaper.GetTrack(0,i),0) end
  line('complete=1');log:close()
  local f=assert(io.open(dir..'/transport.done','w'));f:write('play tempo seek loop stop Free; scripted actual host\n');f:close();return
 end
 reaper.defer(step)
end
reaper.defer(step)
