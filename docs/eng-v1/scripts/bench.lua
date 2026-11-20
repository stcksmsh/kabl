-- REAPER code-module benchmark, one run: wait until kabl's fuel calibration has finished (it
-- writes KABL_CODE_CALIBRATION_REPORT about 2 s after the device starts), then render the master
-- mix offline with the host's own renderer and record how long the render took.
-- Driven by docs/eng-v1/scripts/reaper_code_bench.py.
local dir=assert(os.getenv('KB_DIR'))
local tag=assert(os.getenv('KB_TAG'))
local secs=tonumber(assert(os.getenv('KB_SECS')))
local report=assert(os.getenv('KABL_CODE_CALIBRATION_REPORT'))
local started=reaper.time_precise()
local function exists(p) local f=io.open(p,'r') if f then f:close() return true end return false end
local function step()
 local waited=reaper.time_precise()-started
 if waited<3 or (not exists(report) and waited<30) then reaper.defer(step) return end
 reaper.OnStopButton()
 for i=0,reaper.CountTracks(0)-1 do
  local tr=reaper.GetTrack(0,i)
  reaper.TrackFX_Show(tr,0,2)
  assert(reaper.TrackFX_GetCount(tr)==1 and not reaper.TrackFX_GetOffline(tr,0),'plugin did not load')
 end
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
 local t0=reaper.time_precise()
 reaper.Main_OnCommand(42230,0)
 local render_s=reaper.time_precise()-t0
 local wav=assert(io.open(dir..'/'..name..'.wav','rb'));local bytes=wav:seek('end');wav:close()
 assert(bytes>=secs*48000*2*4,'truncated render')
 local _,bsize=reaper.GetAudioDeviceInfo('BSIZE')
 local done=assert(io.open(dir..'/done-'..tag,'w'))
 done:write('render_s=',render_s,'\ncalibrated=',tostring(exists(report)),'\nbsize=',bsize,'\nreaper=',reaper.GetAppVersion(),'\n')
 done:close()
end
reaper.defer(step)
