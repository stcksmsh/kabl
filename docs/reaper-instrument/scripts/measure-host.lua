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
 other=other:gsub('<FXCHAIN.-\n%s*>\n%s*<ITEM',function() return fx..'\n<ITEM' end,1)
 assert(reaper.SetTrackStateChunk(tr,other,false))
 reaper.TrackFX_SetOffline(tr,0,i>=count)
 reaper.TrackFX_Show(tr,0,opened and i<count and 3 or 2)
 reaper.SetMediaTrackInfo_Value(tr,'I_PERFFLAGS',3) -- disable buffering and anticipative FX
end
reaper.SetEditCurPos(0,false,false)
reaper.OnPlayButton()
local start=reaper.time_precise()
local f=assert(io.open(dir..'/measure-device.txt','w'))
for _,key in ipairs({'SRATE','BSIZE','MODE'}) do local ok,v=reaper.GetAudioDeviceInfo(key);f:write(key,'=',tostring(ok),' ',v,'\n') end
f:close()
local function tick()
 if reaper.time_precise()-start<12 then reaper.defer(tick);return end
 reaper.OnStopButton()
 for i=0,1 do reaper.TrackFX_Show(reaper.GetTrack(0,i),0,2) end
 local done=assert(io.open(dir..'/measure.done','w'));done:write(reaper.time_precise()-start,'\n');done:close()
end
reaper.defer(tick)
