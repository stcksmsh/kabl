-- Real rack gestures are driven through X11; this script only manages the host.
local dir=assert(os.getenv('KABL_D08_HOST'))
reaper.Audio_Init();reaper.OnStopButton()
local tr=reaper.GetTrack(0,0)
reaper.TrackFX_Show(reaper.GetTrack(0,1),0,2)
reaper.TrackFX_Show(tr,0,3)
reaper.SetMediaTrackInfo_Value(tr,'I_AUTOMODE',3)
reaper.SetMediaItemInfo_Value(reaper.GetTrackMediaItem(tr,0),'D_LENGTH',75)
reaper.SetEditCurPos(0,false,false)
local start=reaper.time_precise();local playing=false;local gesture=0;local ready=false
local function step()
 local elapsed=reaper.time_precise()-start
 if not ready then
  if elapsed<7 then reaper.defer(step);return end
  -- Dismiss the ordinary evaluation dialog after its countdown; do not alter licensing.
  os.execute("DISPLAY=:110 xdotool search --onlyvisible --name '^About REAPER' windowactivate --sync key Escape 2>/dev/null")
  for i=0,1 do reaper.TrackFX_Show(reaper.GetTrack(0,i),0,0);reaper.TrackFX_Show(reaper.GetTrack(0,i),0,2) end
  reaper.TrackFX_Show(tr,0,3)
  os.execute("DISPLAY=:110 xdotool search --onlyvisible --name '^CLAPi:.*kabl' windowmove 20 10")
  start=reaper.time_precise();ready=true;reaper.defer(step);return
 end
 if not playing and elapsed>1 then
  os.execute("DISPLAY=:110 xdotool search --onlyvisible --name '^CLAPi:.*kabl' windowmove --sync 20 10")
  reaper.OnPlayButton();playing=true
 end
 if gesture==0 and elapsed>3 then
  os.execute('DISPLAY=:110 xdotool mousemove 225 106 click 1');gesture=1
 elseif gesture==1 and elapsed>4 then
  os.execute('DISPLAY=:110 xdotool mousemove 220 151 mousedown 1 mousemove --sync 252 151 sleep 0.5 mousemove --sync 188 151 sleep 0.5 mousemove --sync 252 151 mouseup 1');gesture=2
 elseif gesture==2 and elapsed>6 then
  os.execute('DISPLAY=:110 import -window root "'..dir..'/rack-write.png"');gesture=3
 end
 if elapsed<10 then reaper.defer(step);return end
 reaper.OnStopButton();reaper.SetMediaTrackInfo_Value(tr,'I_AUTOMODE',1)
 local env=assert(reaper.GetFXEnvelope(tr,0,1,false))
 local f=assert(io.open(dir..'/rack-record-host.txt','w'))
 f:write('scripted X11 gestures; REAPER Write mode; points=',reaper.CountEnvelopePoints(env),'\n')
 for n=0,reaper.CountEnvelopePoints(env)-1 do
  local ok,t,v,shape,tension,selected=reaper.GetEnvelopePoint(env,n);assert(ok)
  f:write(t,' ',v,' ',shape,'\n')
 end
 local changed=false
 for n=0,reaper.CountEnvelopePoints(env)-1 do local ok,t,v=reaper.GetEnvelopePoint(env,n);if ok and t>3 and t<6 and v>.7 then changed=true end end
 assert(changed,'scripted rack gesture was not recorded')
 f:close();reaper.Main_SaveProjectEx(0,dir..'/rack-recorded.rpp',0)
 local done=assert(io.open(dir..'/rack-record.done','w'));done:write('recorded rack gestures\n');done:close()
end
reaper.defer(step)
