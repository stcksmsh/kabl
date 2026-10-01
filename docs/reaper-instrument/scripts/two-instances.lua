-- Run only in the D07 scratch project after save-edited.lua and save-browser.lua.
local dir=assert(os.getenv('KABL_D07_HOST'))
assert(reaper.CountTracks(0)==1,'expected one scratch browser track')
reaper.OnStopButton()
local file=assert(io.open(dir..'/edited.rpp','r'))
local text=file:read('*a');file:close()
local lines={};local depth=0;local started=false
for line in text:gmatch('[^\r\n]+') do
 if not started and line:match('^%s*<TRACK') then started=true end
 if started then
  lines[#lines+1]=line
  if line:match('^%s*<') then depth=depth+1 end
  if line:match('^%s*>%s*$') then depth=depth-1 end
  if depth==0 then break end
 end
end
local chunk=table.concat(lines,'\n'):gsub('TRACKID {%S-}', 'TRACKID '..reaper.genGuid()):gsub('FXID {%S-}', 'FXID '..reaper.genGuid())
reaper.InsertTrackAtIndex(0,true)
assert(reaper.SetTrackStateChunk(reaper.GetTrack(0,0),chunk,false),'edited instance state rejected')
local names={'D07 poly: cutoff + right routing','D07 expressive lead: browser + expression'}
local takes={}
for index=0,1 do
 local track=reaper.GetTrack(0,index)
 reaper.GetSetMediaTrackInfo_String(track,'P_NAME',names[index+1],true)
 while reaper.CountTrackMediaItems(track)>0 do reaper.DeleteTrackMediaItem(track,reaper.GetTrackMediaItem(track,0)) end
 reaper.SetMediaTrackInfo_Value(track,'D_VOL',index==0 and 0.7 or 0.24)
 reaper.TrackFX_Show(track,0,2)
 local item=reaper.CreateNewMIDIItemInProj(track,0,52,false)
 takes[index+1]=reaper.GetActiveTake(item)
end
local function ppq(take,time) return reaper.MIDI_GetPPQPosFromProjTime(take,time) end
local function note(take,start,length,channel,key,velocity)
 reaper.MIDI_InsertNote(take,false,false,ppq(take,start),ppq(take,start+length),channel,key,velocity,true)
end
local function cc(take,time,status,channel,a,b)
 reaper.MIDI_InsertCC(take,false,false,ppq(take,time),status,channel,a,b)
end
local roots={60,57,53,55}
for bar=0,13 do
 local start=0.5+bar*2
 local root=roots[bar%4+1]
 for _,interval in ipairs({0,4,7}) do note(takes[1],start,1.2,0,root+interval,72) end
 cc(takes[1],start+0.1,176,0,64,127)
 cc(takes[1],start+1.7,176,0,64,0)
end
local melody={72,76,79,76,69,72,76,74,65,69,72,69,67,71,74,79}
for step=0,55 do note(takes[2],0.5+step*0.5,0.43,1,melody[step%#melody+1],88) end
for step=0,111 do
 local time=0.5+step*0.25
 local bend=math.floor(8192+math.sin(step*0.31)*3500)
 cc(takes[2],time,224,1,bend%128,math.floor(bend/128))
 cc(takes[2],time,176,1,1,math.floor(50+50*math.sin(step*0.12)))
end
for channel=0,1 do
 cc(takes[channel+1],29,176,channel,64,0)
 cc(takes[channel+1],29,176,channel,123,0)
 cc(takes[channel+1],29,176,channel,121,0)
end
for _,take in ipairs(takes) do reaper.MIDI_Sort(take) end
reaper.GetSetProjectInfo_String(0,'RENDER_FILE',dir,true)
reaper.GetSetProjectInfo_String(0,'RENDER_PATTERN','d07-musical',true)
reaper.GetSetProjectInfo(0,'RENDER_BOUNDSFLAG',0,true)
reaper.GetSetProjectInfo(0,'RENDER_STARTPOS',0,true)
reaper.GetSetProjectInfo(0,'RENDER_ENDPOS',52,true)
reaper.GetSetProjectInfo(0,'RENDER_SRATE',48000,true)
reaper.GetSetProjectInfo(0,'RENDER_CHANNELS',2,true)
reaper.GetSetProjectInfo(0,'RENDER_TAILFLAG',0,true)
reaper.GetSetProjectInfo(0,'RENDER_SETTINGS',0,true)
reaper.GetSetProjectInfo_String(0,'RENDER_FORMAT','ZXZhdyAAAA==',true)
reaper.Main_SaveProjectEx(0,dir..'/two-instances.rpp',0)
local f=assert(io.open(dir..'/two-instances.txt','w'))
for index=0,1 do f:write('track ',index,' gain=',reaper.TrackFX_GetParamNormalized(reaper.GetTrack(0,index),0,0),' fx=',reaper.TrackFX_GetCount(reaper.GetTrack(0,index)),'\n') end
f:write('scripted MIDI; music 0.5..29 s; render 0..52 s; editors closed\n');f:close()
reaper.SetEditCurPos(0,false,false)
reaper.OnPlayButton()
