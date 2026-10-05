-- Measurement scratch only: existing two-track MIDI project, no saved edits.
local dir=assert(os.getenv('KABL_D08_HOST'))
local kind=assert(os.getenv('D08_MEASURE_KIND'))
local count=tonumber(assert(os.getenv('D08_MEASURE_COUNT')))
local opened=os.getenv('D08_MEASURE_OPEN')=='1'
assert(reaper.CountTracks(0)==2)
reaper.OnStopButton()
local source=reaper.GetTrack(0,kind=='light' and 0 or 1)
-- Use the host's native FX copy; complete state and unique identity are preserved.
for i=0,1 do
 local tr=reaper.GetTrack(0,i)
 if tr~=source then
  reaper.TrackFX_Delete(tr,0)
  reaper.TrackFX_CopyToTrack(source,0,tr,0,false)
 end
 assert(reaper.TrackFX_GetCount(tr)==1)
 reaper.TrackFX_SetOffline(tr,0,i>=count)
 reaper.TrackFX_Show(tr,0,0);reaper.TrackFX_Show(tr,0,2)
 reaper.SetMediaTrackInfo_Value(tr,'I_PERFFLAGS',3)
end
local f=assert(io.open(dir..'/measure-device.txt','w'))
for _,key in ipairs({'SRATE','BSIZE','MODE'}) do local ok,v=reaper.GetAudioDeviceInfo(key);f:write(key,'=',tostring(ok),' ',v,'\n') end
local handles=assert(io.open(dir..'/gui-handles.txt','w'))
local checks=0
local function check_handles()
 for i=0,1 do
  local tr=reaper.GetTrack(0,i)
  local handle=reaper.TrackFX_GetFloatingWindow(tr,0)
  assert(reaper.TrackFX_GetChainVisible(tr)==-1,"FX chain editor must be hidden")
  local expected=opened and i<count
  handles:write('check=',checks,' track=',i,' expected=',tostring(expected),' handle=',tostring(handle),'\n');handles:flush()
  assert((handle~=nil)==expected,'unexpected actual floating GUI handle')
 end
 checks=checks+1
end
-- Both instances receive the same scripted played MIDI; duplicate the embedded item.
local item=reaper.GetTrackMediaItem(reaper.GetTrack(0,0),0)
local ok,item_chunk=reaper.GetItemStateChunk(item,'',false);assert(ok)
item_chunk=item_chunk:gsub('{[%x%-]+}',function() return reaper.genGuid() end)
local second=reaper.AddMediaItemToTrack(reaper.GetTrack(0,1));assert(reaper.SetItemStateChunk(second,item_chunk,false))
reaper.Audio_Init()
local initialized=reaper.time_precise()
local start
local function tick()
 if reaper.time_precise()-initialized<1 then reaper.defer(tick);return end
 if not start then
  for i=0,1 do
   local tr=reaper.GetTrack(0,i)
   reaper.TrackFX_Show(tr,0,0);reaper.TrackFX_Show(tr,0,2)
   if opened and i<count then reaper.TrackFX_Show(tr,0,3) end
  end
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
