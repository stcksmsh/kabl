local dir=assert(os.getenv('KABL_D07_HOST'))
local track=reaper.GetTrack(0,0)
reaper.TrackFX_SetParamNormalized(track,0,0,0.55)
local start=reaper.time_precise()
local function save()
 if reaper.time_precise()-start<1 then reaper.defer(save); return end
 reaper.Main_SaveProjectEx(0,dir..'/browser-loaded.rpp',0)
end
reaper.defer(save)
