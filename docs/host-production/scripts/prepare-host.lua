-- Actual host fixture and readback. Input remains scripted/virtual.
local dir=assert(os.getenv('KABL_D08_HOST'))
local log=assert(io.open(dir..'/host-prepare.txt','w'))
local function write(s) log:write(tostring(s),'\n');log:flush() end
assert(reaper.CountTracks(0)==2)
write('REAPER '..reaper.GetAppVersion())
for t=0,1 do
 local tr=reaper.GetTrack(0,t)
 assert(reaper.TrackFX_GetCount(tr)==1)
 for p=0,reaper.TrackFX_GetNumParams(tr,0)-1 do
  local ok,name=reaper.TrackFX_GetParamName(tr,0,p)
  write(t..' '..p..' '..name..' '..reaper.TrackFX_GetParamNormalized(tr,0,p))
 end
end
reaper.SetTempoTimeSigMarker(0,-1,0,-1,-1,120,4,4,false)
reaper.SetTempoTimeSigMarker(0,-1,8,-1,-1,140,4,4,false)
reaper.SetTempoTimeSigMarker(0,-1,16,-1,-1,100,4,4,false)
for t=0,1 do
 local tr=reaper.GetTrack(0,t)
 local env=reaper.GetFXEnvelope(tr,0,1,true)
 assert(env)
 for _,point in ipairs({{0,0.4},{4,0.65},{8,0.25},{12,0.75},{16,0.4},{23,0.55}}) do reaper.InsertEnvelopePoint(env,point[1],point[2],0,0,false,true) end
 reaper.Envelope_SortPoints(env)
 reaper.SetMediaTrackInfo_Value(tr,'I_AUTOMODE',1)
end
local env=reaper.GetFXEnvelope(reaper.GetTrack(0,1),0,2,true)
assert(env)
reaper.InsertEnvelopePoint(env,0,1,1,0,false,true)
reaper.InsertEnvelopePoint(env,23.9,1,1,0,false,true)
reaper.InsertEnvelopePoint(env,24,0,1,0,false,true)
reaper.Envelope_SortPoints(env)
reaper.Main_SaveProjectEx(0,dir..'/d08-arrangement.rpp',0)
reaper.TrackFX_Show(reaper.GetTrack(0,0),0,3)
write('floating='..tostring(reaper.TrackFX_GetFloatingWindow(reaper.GetTrack(0,0),0)))
write('complete=1')
log:close()
