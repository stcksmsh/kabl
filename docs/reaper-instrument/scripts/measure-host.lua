-- Measurement scratch only: existing two-track MIDI project, no saved edits.
local dir=assert(os.getenv('KABL_D07_HOST'))
local kind=assert(os.getenv('KABL_MEASURE_KIND'))
local count=tonumber(assert(os.getenv('KABL_MEASURE_COUNT')))
local opened=os.getenv('KABL_MEASURE_OPEN')=='1'
assert(reaper.CountTracks(0)==2)
reaper.OnStopButton()
local source=reaper.GetTrack(0,kind=='light' and 0 or 1)
-- Copy the selected sound into both tracks; retain each track's scripted MIDI.
local ok,chunk=reaper.GetTrackStateChunk(source,'',false);assert(ok)
local fx=assert(chunk:match('(<FXCHAIN.-\n%s*>\n%s*<ITEM)')):gsub('\n%s*<ITEM$','')
for i=0,1 do
 local tr=reaper.GetTrack(0,i)
 local good,other=reaper.GetTrackStateChunk(tr,'',false);assert(good)
 local instance_fx=fx:gsub('FXID {%S-}',function() return 'FXID '..reaper.genGuid() end)
 other=other:gsub('<FXCHAIN.-\n%s*>\n%s*<ITEM',function() return instance_fx..'\n<ITEM' end,1)
 assert(reaper.SetTrackStateChunk(tr,other,false))
 reaper.TrackFX_SetOffline(tr,0,i>=count)
 reaper.TrackFX_Show(tr,0,opened and i<count and 3 or 2)
 reaper.SetMediaTrackInfo_Value(tr,'I_PERFFLAGS',3) -- disable buffering and anticipative FX
end
local f=assert(io.open(dir..'/measure-device.txt','w'))
for _,key in ipairs({'SRATE','BSIZE','MODE'}) do local ok,v=reaper.GetAudioDeviceInfo(key);f:write(key,'=',tostring(ok),' ',v,'\n') end
local handles=assert(io.open(dir..'/gui-handles.txt','w'))
local checks=0
local function check_handles()
 for i=0,1 do
  local tr=reaper.GetTrack(0,i)
  local handle=reaper.TrackFX_GetFloatingWindow(tr,0)
  local expected=opened and i<count
  handles:write('check=',checks,' track=',i,' expected=',tostring(expected),' handle=',tostring(handle),'\n');handles:flush()
  assert((handle~=nil)==expected,'unexpected actual floating GUI handle')
 end
 checks=checks+1
end
local start
local function tick()
 if not start then
  check_handles()
  reaper.Main_SaveProjectEx(0,dir..'/measured-project.rpp',0)
  reaper.SetEditCurPos(0,false,false)
  reaper.OnPlayButton()
  start=reaper.time_precise()
 end
 if reaper.time_precise()-start<12 then
  if checks==1 and reaper.time_precise()-start>=6 then check_handles() end
  reaper.defer(tick);return
 end
 check_handles()
 local duration=reaper.time_precise()-start
 f:write('duration=',duration,'\nGUI_checks=',checks,'\n');f:close();handles:close()
 reaper.OnStopButton()
 for i=0,1 do reaper.TrackFX_Show(reaper.GetTrack(0,i),0,2) end
 local marker=dir..'/measure.done'
 local done=assert(io.open(marker..'.tmp','w'));done:write(duration,'\n');done:close();assert(os.rename(marker..'.tmp',marker))
end
reaper.defer(tick)
