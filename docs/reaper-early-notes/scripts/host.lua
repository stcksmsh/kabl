-- KABL_SE_MODE=realtime records the master output while the project plays at real-time speed
-- (track record mode "output") instead of rendering offline; the recorded file is named in done-<tag>.
-- Real-REAPER pass for the sound-engines check: report the loaded plugin, save the project (the
-- plugin's state is written into it), then render the master mix offline with the host's own
-- renderer. Same render settings as docs/host-production/scripts/render.lua.
local dir=assert(os.getenv('KABL_SE_HOST'))
local tag=assert(os.getenv('KABL_SE_TAG'))
local secs=tonumber(os.getenv('KABL_SE_SECS') or '24')
local started=reaper.time_precise()
local function step()
 if reaper.time_precise()-started<tonumber(os.getenv('KABL_SE_DELAY') or '2') then reaper.defer(step) return end
 reaper.OnStopButton()
 local info=assert(io.open(dir..'/info-'..tag..'.txt','w'))
 info:write('REAPER=',reaper.GetAppVersion(),'\n')
 for i=0,reaper.CountTracks(0)-1 do
  local tr=reaper.GetTrack(0,i)
  reaper.TrackFX_Show(tr,0,2)
  local _,name=reaper.TrackFX_GetFXName(tr,0,'')
  info:write('track=',i,' fx_count=',reaper.TrackFX_GetCount(tr),' fx=',name,
   ' offline=',tostring(reaper.TrackFX_GetOffline(tr,0)),' params=',reaper.TrackFX_GetNumParams(tr,0),'\n')
  assert(reaper.TrackFX_GetCount(tr)==1 and not reaper.TrackFX_GetOffline(tr,0),'plugin did not load')
 end
 info:close()
 reaper.Main_SaveProjectEx(0,dir..'/saved-'..tag..'.rpp',0)
 local name='render-'..tag
 if os.getenv('KABL_SE_MODE')=='realtime' then
  -- an armed track does not play its own items back, so record a second track fed by a send
  local src=reaper.GetTrack(0,0)
  reaper.InsertTrackAtIndex(1,false)
  local tr=reaper.GetTrack(0,1)
  reaper.CreateTrackSend(src,tr)
  reaper.SetMediaTrackInfo_Value(tr,'I_RECARM',1)
  reaper.SetMediaTrackInfo_Value(tr,'I_RECMODE',tonumber(os.getenv('RECMODE') or '1'))
  reaper.SetEditCurPos(0,false,false)
  reaper.Main_OnCommand(1013,0)
  local t0=reaper.time_precise()
  local function wait()
   if reaper.time_precise()-t0<secs then reaper.defer(wait) return end
   reaper.Main_OnCommand(40667,0)
   local ok,err=pcall(function()
    local item=reaper.GetTrackMediaItem(tr,reaper.CountTrackMediaItems(tr)-1)
    local src=reaper.GetMediaItemTake_Source(reaper.GetActiveTake(item))
    local done=assert(io.open(dir..'/done-'..tag,'w'));done:write(reaper.GetMediaSourceFileName(src,''),'\n');done:close()
   end)
   if not ok then local e=io.open(dir..'/lua-error','w');e:write(tostring(err));e:close() end
  end
  reaper.defer(wait)
  return
 end
 reaper.GetSetProjectInfo_String(0,'RENDER_FILE',dir,true)
 reaper.GetSetProjectInfo_String(0,'RENDER_PATTERN',name,true)
 reaper.GetSetProjectInfo(0,'RENDER_BOUNDSFLAG',0,true)
 reaper.GetSetProjectInfo(0,'RENDER_STARTPOS',0,true)
 reaper.GetSetProjectInfo(0,'RENDER_ENDPOS',secs,true)
 reaper.GetSetProjectInfo(0,'RENDER_SRATE',48000,true)
 reaper.GetSetProjectInfo(0,'RENDER_CHANNELS',2,true)
 reaper.GetSetProjectInfo(0,'RENDER_TAILFLAG',0,true)
 reaper.GetSetProjectInfo(0,'RENDER_SETTINGS',0,true)
 reaper.GetSetProjectInfo_String(0,'RENDER_FORMAT','ZXZhdyAAAA==',true)
 reaper.Main_OnCommand(42230,0)
 local wav=assert(io.open(dir..'/'..name..'.wav','rb'));local bytes=wav:seek('end');wav:close()
 assert(bytes>=secs*48000*2*4,'truncated render')
 local done=assert(io.open(dir..'/done-'..tag,'w'));done:write('bytes=',bytes,'\n');done:close()
end
reaper.defer(step)
