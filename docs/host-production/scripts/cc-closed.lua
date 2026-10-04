local dir=assert(os.getenv('KABL_D08_HOST'))
reaper.Audio_Init();reaper.OnStopButton()
for i=0,1 do reaper.TrackFX_Show(reaper.GetTrack(0,i),0,0);reaper.TrackFX_Show(reaper.GetTrack(0,i),0,2) end
local tr=reaper.GetTrack(0,1)
reaper.SetOnlyTrackSelected(tr)
reaper.SetMediaTrackInfo_Value(tr,'I_RECINPUT',4096+62*32)
reaper.SetMediaTrackInfo_Value(tr,'I_RECARM',1)
reaper.SetMediaTrackInfo_Value(tr,'I_RECMON',1)
reaper.SetMediaTrackInfo_Value(tr,'I_AUTOMODE',3)
local env=assert(reaper.GetFXEnvelope(tr,0,1,true))
local events={{1.5,20,0},{2,20,51},{2.5,20,90},{3,25,0},{3.5,25,127},{4,25,0},{4.5,20,30}}
local start=reaper.time_precise();local index=1;local playing=false
local f=assert(io.open(dir..'/cc-host.txt','w'))
f:write('source=REAPER virtual keyboard queue; no physical controller; both editors closed\n')
local function step()
 local elapsed=reaper.time_precise()-start
 if not playing and elapsed>1 then
  for i=0,1 do reaper.TrackFX_Show(reaper.GetTrack(0,i),0,0);reaper.TrackFX_Show(reaper.GetTrack(0,i),0,2) end
  reaper.SetEditCurPos(0,false,false);reaper.OnPlayButton();playing=true end
 while index<=#events and elapsed>=events[index][1] do
  for i=0,1 do assert(reaper.TrackFX_GetFloatingWindow(reaper.GetTrack(0,i),0)==nil and reaper.TrackFX_GetChainVisible(reaper.GetTrack(0,i))==-1) end
  local e=events[index];reaper.StuffMIDIMessage(0,176,e[2],e[3]);f:write(elapsed,' CC ',e[2],' ',e[3],'\n');f:flush();index=index+1
 end
 if elapsed<6 then reaper.defer(step);return end
 local value=reaper.TrackFX_GetParamNormalized(tr,0,1)
 f:write('slot settled=',value,' points=',reaper.CountEnvelopePoints(env),'\n');assert(math.abs(value-30/127)<.02,'CC slot not updated')
 reaper.OnStopButton();reaper.SetMediaTrackInfo_Value(tr,'I_AUTOMODE',1)
 reaper.SetMediaTrackInfo_Value(tr,'I_RECARM',0);reaper.SetMediaTrackInfo_Value(tr,'I_RECMON',0)
 reaper.Main_SaveProjectEx(0,dir..'/cc-recorded.rpp',0)
 reaper.TrackFX_Show(tr,0,3)
 local function finish()
  if reaper.time_precise()-start<9 then reaper.defer(finish);return end
  os.execute("DISPLAY=:110 xdotool search --onlyvisible --name '^About REAPER' windowactivate --sync key Escape 2>/dev/null")
  os.execute("DISPLAY=:110 xdotool search --onlyvisible --name '^CLAPi:.*kabl' windowmove --sync 20 10")
  os.execute('DISPLAY=:110 xdotool mousemove 1173 193 click 1 mousemove 682 128 click 1 sleep 0.5 mousemove 40 60')
  os.execute('DISPLAY=:110 import -window root "'..dir..'/cc-button-result.png"')
  f:write('clock opened only after all messages and state save\n');f:close()
  local done=assert(io.open(dir..'/cc.done','w'));done:write('closed-editor CC complete\n');done:close()
 end
 reaper.defer(finish)
end
reaper.defer(step)
