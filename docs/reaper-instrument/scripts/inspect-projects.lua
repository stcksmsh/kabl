local dir = assert(os.getenv('KABL_D07_HOST'))
local f = assert(io.open(dir .. '/instance-projects.txt','w'))
f:write('resource=',reaper.GetResourcePath(),'\n')
local index=0
while true do
 local project,path = reaper.EnumProjects(index,'')
 if not project then break end
 f:write('project=',index,' path=',path,' dirty=',reaper.IsProjectDirty(project),' tracks=',reaper.CountTracks(project),'\n')
 index=index+1
end
f:close()
