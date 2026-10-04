local dir = assert(os.getenv('KABL_D07_HOST'))
local f = assert(io.open(dir .. '/info.txt', 'w'))
f:write('REAPER ', reaper.GetAppVersion(), '\n')
for _, key in ipairs({'SRATE', 'BSIZE', 'MODE', 'IDENT_IN', 'IDENT_OUT'}) do
  local ok, value = reaper.GetAudioDeviceInfo(key)
  f:write(key, ' ', tostring(ok), ' ', value, '\n')
end
f:close()
