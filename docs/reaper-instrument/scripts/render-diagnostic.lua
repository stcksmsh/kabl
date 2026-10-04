-- Scratch only. Identical saved state; no editor changes or live MIDI injection.
local dir=assert(os.getenv('KABL_D07_HOST'))
local name=assert(os.getenv('KABL_RENDER_NAME'))
assert(reaper.CountTracks(0)==2)
reaper.OnStopButton()
for i=0,1 do
 local tr=reaper.GetTrack(0,i)
 reaper.TrackFX_Show(tr,0,2)
 assert(reaper.TrackFX_GetCount(tr)==1)
end
reaper.GetSetProjectInfo_String(0,'RENDER_FILE',dir,true)
reaper.GetSetProjectInfo_String(0,'RENDER_PATTERN',name,true)
reaper.GetSetProjectInfo(0,'RENDER_BOUNDSFLAG',0,true)
reaper.GetSetProjectInfo(0,'RENDER_STARTPOS',0,true)
reaper.GetSetProjectInfo(0,'RENDER_ENDPOS',52,true)
reaper.GetSetProjectInfo(0,'RENDER_SRATE',48000,true)
reaper.GetSetProjectInfo(0,'RENDER_CHANNELS',2,true)
reaper.GetSetProjectInfo(0,'RENDER_TAILFLAG',0,true)
reaper.GetSetProjectInfo(0,'RENDER_SETTINGS',0,true)
reaper.GetSetProjectInfo_String(0,'RENDER_FORMAT','ZXZhdyAAAA==',true)
local mode=os.getenv('KABL_RENDER_POLICY') or 'fresh-process'
if mode=='serial-offline' then
 for i=0,1 do reaper.SetMediaTrackInfo_Value(reaper.GetTrack(0,i),'I_PERFFLAGS',3) end
end
local audio_before=reaper.Audio_IsRunning()
if mode=='device-closed' or mode=='serial-offline' then
 reaper.Audio_Quit()
 assert(reaper.Audio_IsRunning()==0,'audio device did not stop')
end
if mode=='offline-online' or mode=='device-closed' or mode=='serial-offline' then
 for i=0,1 do local tr=reaper.GetTrack(0,i);reaper.TrackFX_SetOffline(tr,0,true);reaper.TrackFX_SetOffline(tr,0,false) end
end
local start=reaper.time_precise()
reaper.Main_OnCommand(42230,0)
local wav=assert(io.open(dir..'/'..name..'.wav','rb'),'render did not produce file')
local bytes=wav:seek('end');wav:close()
assert(bytes>=52*48000*2*4,'render truncated')
local marker=dir..'/'..name..'.done'
local f=assert(io.open(marker..'.tmp','w'))
f:write('policy=',mode,'\nseconds=',reaper.time_precise()-start,'\nbytes=',bytes,'\naudio_before=',audio_before,'\naudio_after_render=',reaper.Audio_IsRunning(),'\n')
f:close()
assert(os.rename(marker..'.tmp',marker))

if mode=='device-closed' or mode=='serial-offline' then reaper.Audio_Init() end
