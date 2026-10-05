local dir=assert(os.getenv('KABL_D08_HOST'))
reaper.Audio_Init();reaper.OnStopButton()
reaper.TrackFX_Show(reaper.GetTrack(0,0),0,2)
reaper.TrackFX_Show(reaper.GetTrack(0,1),0,3)
local start=reaper.time_precise();local phase=0
local function step()
 local elapsed=reaper.time_precise()-start
 if phase==0 and elapsed>7 then
  os.execute("DISPLAY=:110 xdotool search --onlyvisible --name '^About REAPER' windowactivate --sync key Escape 2>/dev/null")
  for i=0,1 do reaper.TrackFX_Show(reaper.GetTrack(0,i),0,0);reaper.TrackFX_Show(reaper.GetTrack(0,i),0,2) end
  reaper.TrackFX_Show(reaper.GetTrack(0,1),0,3)
  assert(reaper.TrackFX_GetFloatingWindow(reaper.GetTrack(0,1),0)~=nil)
  os.execute("DISPLAY=:110 xdotool search --onlyvisible --name '^CLAPi:.*kabl' windowmove 20 10")
  phase=1
 elseif phase==1 and elapsed>8 then
  os.execute("DISPLAY=:110 xdotool search --onlyvisible --name '^CLAPi:.*kabl' windowmove --sync 20 10")
  os.execute('DISPLAY=:110 xdotool mousemove 1173 193 click 1 mousemove 682 128 click 1 sleep 0.5 mousemove 40 60')
  os.execute('DISPLAY=:110 import -window root "'..dir..'/ui-light-'..assert(os.getenv('D08_SCREEN') or '1440x900')..'.png"');phase=2
 elseif phase==2 and elapsed>10 then
  os.execute('DISPLAY=:110 xdotool mousemove 840 128 click 1 sleep 0.5 mousemove 40 60')
  os.execute('DISPLAY=:110 import -window root "'..dir..'/ui-dark-'..assert(os.getenv('D08_SCREEN') or '1440x900')..'.png"');phase=3
 elseif phase==3 and elapsed>11 then
  local f=assert(io.open(dir..'/ui.done','w'));f:write('actual GUI handle verified; fitted light/dark views\n');f:close();return
 end
 reaper.defer(step)
end
reaper.defer(step)
