-- REAPER virtual keyboard queue, not a physical controller or prewritten MIDI item.
local dir=assert(os.getenv('KABL_D07_HOST'))
assert(reaper.CountTracks(0)==2,'expected scratch two-instance project')
reaper.OnStopButton()
reaper.InsertTrackAtIndex(2,true)
local track=reaper.GetTrack(0,2)
reaper.TrackFX_CopyToTrack(reaper.GetTrack(0,0),0,track,0,false)
reaper.GetSetMediaTrackInfo_String(track,'P_NAME','D07 recorded virtual keyboard input',true)
for index=0,1 do reaper.SetMediaTrackInfo_Value(reaper.GetTrack(0,index),'B_MUTE',1) end
reaper.SetMediaTrackInfo_Value(track,'I_RECINPUT',4096+62*32)
reaper.SetMediaTrackInfo_Value(track,'I_RECMODE',0)
reaper.SetMediaTrackInfo_Value(track,'I_RECARM',1)
reaper.SetMediaTrackInfo_Value(track,'I_RECMON',1)
reaper.SetOnlyTrackSelected(track)
reaper.SetEditCurPos(0,false,false)
local events={
 {0.3,144,60,100},{0.5,176,64,127},{1.0,128,60,0},
 {1.3,224,0,96},{1.5,176,1,100},{1.7,144,64,95},{2.5,128,64,0},
 {2.8,176,64,0},{3.2,144,60,90},{3.2,144,64,90},{3.2,144,67,90},
 {4.3,128,60,0},{4.3,128,64,0},{4.3,128,67,0},{4.6,176,123,0},{4.7,176,121,0},
}
local log=assert(io.open(dir..'/virtual-record.txt','w'))
log:write('source=REAPER StuffMIDIMessage mode 0 virtual keyboard; no physical controller\n')
reaper.CSurf_OnRecord()
local start=reaper.time_precise();local index=1
local function step()
 local elapsed=reaper.time_precise()-start
 while index<=#events and elapsed>=events[index][1] do
  local event=events[index];reaper.StuffMIDIMessage(0,event[2],event[3],event[4])
  log:write('sent ',elapsed,' ',event[2],' ',event[3],' ',event[4],'\n');log:flush();index=index+1
 end
 if elapsed<5.5 then reaper.defer(step);return end
 reaper.OnStopButton()
 reaper.SetMediaTrackInfo_Value(track,'I_RECARM',0)
 reaper.SetMediaTrackInfo_Value(track,'I_RECMON',0)
 log:write('recorded items=',reaper.CountTrackMediaItems(track),'\n')
 for item_index=0,reaper.CountTrackMediaItems(track)-1 do
  local take=reaper.GetActiveTake(reaper.GetTrackMediaItem(track,item_index))
  local ok,notes,cc,text=reaper.MIDI_CountEvts(take)
  log:write('take notes=',notes,' cc=',cc,' text=',text,'\n')
 end
 reaper.Main_SaveProjectEx(0,dir..'/virtual-recording.rpp',0)
 log:close()
end
reaper.defer(step)
