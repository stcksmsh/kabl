-- Real-REAPER pass for the sound-engines check: report the loaded plugin, save the project (the
-- plugin's state is written into it), then render the master mix offline with the host's own
-- renderer. Same render settings as docs/host-production/scripts/render.lua.
local dir=assert(os.getenv('KABL_SE_HOST'))
local tag=assert(os.getenv('KABL_SE_TAG'))
local secs=tonumber(os.getenv('KABL_SE_SECS') or '24')
local started=reaper.time_precise()
local function step()
 if reaper.time_precise()-started<2 then reaper.defer(step) return end
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
