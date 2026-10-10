-- Real-REAPER pass for the rhythm batch (arp, swing, ratchets): optionally put a tempo change in
-- the project (KABL_SE_TEMPO=1: 120 bpm until 12 s, then 150), report the plugin, save the project
-- (the plugin's state is written into it), then render [KABL_SE_START, KABL_SE_START+KABL_SE_SECS]
-- of the master mix offline with the host's own renderer. Same render settings as host.lua.
local dir=assert(os.getenv('KABL_SE_HOST'))
local tag=assert(os.getenv('KABL_SE_TAG'))
local secs=tonumber(os.getenv('KABL_SE_SECS') or '20')
local start=tonumber(os.getenv('KABL_SE_START') or '0')
local started=reaper.time_precise()
local function step()
 if reaper.time_precise()-started<2 then reaper.defer(step) return end
 reaper.OnStopButton()
 if os.getenv('KABL_SE_TEMPO')=='1' and reaper.CountTempoTimeSigMarkers(0)==0 then
  reaper.SetTempoTimeSigMarker(0,-1,12.0,-1,-1,150.0,0,0,false)
  reaper.UpdateTimeline()
 end
 local info=assert(io.open(dir..'/info-'..tag..'.txt','w'))
 info:write('REAPER=',reaper.GetAppVersion(),'\n')
 info:write('tempo_markers=',reaper.CountTempoTimeSigMarkers(0),'\n')
 for i=0,reaper.CountTracks(0)-1 do
  local tr=reaper.GetTrack(0,i)
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
 reaper.GetSetProjectInfo(0,'RENDER_STARTPOS',start,true)
 reaper.GetSetProjectInfo(0,'RENDER_ENDPOS',start+secs,true)
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
