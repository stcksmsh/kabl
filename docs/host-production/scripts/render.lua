-- Completed actual-host offline renders. Every run has a unique retained file.
local dir=assert(os.getenv('KABL_D08_HOST'))
local counter=io.open(dir..'/render-count.txt','r');local n=counter and tonumber(counter:read('*a')) or 0;if counter then counter:close() end
n=n+1;local name='d08-render-'..n
reaper.OnStopButton()
for i=0,reaper.CountTracks(0)-1 do reaper.TrackFX_Show(reaper.GetTrack(0,i),0,2) end
reaper.GetSetProjectInfo_String(0,'RENDER_FILE',dir,true)
reaper.GetSetProjectInfo_String(0,'RENDER_PATTERN',name,true)
reaper.GetSetProjectInfo(0,'RENDER_BOUNDSFLAG',0,true)
reaper.GetSetProjectInfo(0,'RENDER_STARTPOS',0,true)
reaper.GetSetProjectInfo(0,'RENDER_ENDPOS',36,true)
reaper.GetSetProjectInfo(0,'RENDER_SRATE',48000,true)
reaper.GetSetProjectInfo(0,'RENDER_CHANNELS',2,true)
reaper.GetSetProjectInfo(0,'RENDER_TAILFLAG',0,true)
reaper.GetSetProjectInfo(0,'RENDER_SETTINGS',0,true)
reaper.GetSetProjectInfo_String(0,'RENDER_FORMAT','ZXZhdyAAAA==',true)
local start=reaper.time_precise()
reaper.Main_OnCommand(42230,0)
local wav=assert(io.open(dir..'/'..name..'.wav','rb'));local bytes=wav:seek('end');wav:close()
assert(bytes>=36*48000*2*4,'truncated render')
local f=assert(io.open(dir..'/'..name..'.done','w'));f:write('seconds=',reaper.time_precise()-start,'\nbytes=',bytes,'\n');f:close()
f=assert(io.open(dir..'/render-count.txt','w'));f:write(n);f:close()
