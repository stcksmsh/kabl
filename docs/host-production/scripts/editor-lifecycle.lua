-- Actual REAPER GUI and plugin destruction/recreation, with complete-state recall.
local dir=assert(os.getenv('KABL_D08_HOST'))
local log=assert(io.open(dir..'/editor-lifecycle.txt','w'))
local chunks,values={},{}
reaper.Audio_Init();reaper.OnStopButton()
local start=reaper.time_precise();local phase=0
local function show(mode)
 for i=0,1 do reaper.TrackFX_Show(reaper.GetTrack(0,i),0,mode) end
end
local function check(open)
 for i=0,1 do
  local tr=reaper.GetTrack(0,i)
  local handle=reaper.TrackFX_GetFloatingWindow(tr,0)
  log:write('phase=',phase,' track=',i,' floating=',tostring(handle),' chain=',reaper.TrackFX_GetChainVisible(tr),'\n');log:flush()
  assert((handle~=nil)==open)
  assert(reaper.TrackFX_GetChainVisible(tr)==-1)
 end
end
local function step()
 local t=reaper.time_precise()-start
 if phase==0 and t>1 then
  show(0);show(2);check(false);show(3);phase=1
 elseif phase==1 and t>2 then
  check(true);reaper.SetEditCurPos(0,false,false);reaper.OnPlayButton();show(2);phase=2
 elseif phase==2 and t>3 then
  check(false);show(3);phase=3
 elseif phase==3 and t>4 then
  check(true);show(2);phase=4
 elseif phase==4 and t>5 then
  check(false);show(1);phase=5
 elseif phase==5 and t>6 then
  for i=0,1 do assert(reaper.TrackFX_GetChainVisible(reaper.GetTrack(0,i))==0) end
  show(0);show(2);reaper.OnStopButton();phase=6
 elseif phase==6 and t>7 then
  check(false)
  for i=0,1 do
   local tr=reaper.GetTrack(0,i)
   local ok,chunk=reaper.GetTrackStateChunk(tr,'',false);assert(ok);chunks[i]=chunk
   values[i]={};for p=0,17 do values[i][p]=reaper.TrackFX_GetParamNormalized(tr,0,p) end
  end
  reaper.Main_SaveProjectEx(0,dir..'/lifecycle-before.rpp',0)
  for i=0,1 do local tr=reaper.GetTrack(0,i);reaper.TrackFX_Delete(tr,0);assert(reaper.TrackFX_GetCount(tr)==0) end
  log:write('both plugin instances destroyed\n');log:flush();phase=7
 elseif phase==7 and t>8 then
  for i=0,1 do assert(reaper.SetTrackStateChunk(reaper.GetTrack(0,i),chunks[i],false)) end
  show(3);phase=8
 elseif phase==8 and t>10 then
  check(true)
  for i=0,1 do
   local tr=reaper.GetTrack(0,i);assert(reaper.TrackFX_GetCount(tr)==1)
   for p=0,17 do assert(math.abs(reaper.TrackFX_GetParamNormalized(tr,0,p)-values[i][p])<1e-6,'native recall mismatch') end
  end
  reaper.Main_SaveProjectEx(0,dir..'/lifecycle-after.rpp',0)
  reaper.OnPlayButton();phase=9
 elseif phase==9 and t>12 then
  assert(reaper.GetPlayState()~=0);show(2);check(false);reaper.OnStopButton()
  log:write('recreated playback complete; all 18 native values preserved per instance\n');log:close()
  local f=assert(io.open(dir..'/editor-lifecycle.done','w'));f:write('actual GUI + plugin destroy/recreate verified\n');f:close();return
 end
 reaper.defer(step)
end
reaper.defer(step)
