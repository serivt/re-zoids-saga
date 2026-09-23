-- Drive Zoids Saga in mGBA headless: press keys at given frames, take
-- screenshots and dump palette/VRAM/OAM/IO. Configure through env vars:
--   PROBE_OUT   output directory
--   PROBE_PLAN  semicolon-separated "frame:action" items where action is
--               shot:<name> | key:<KEY> | dump:<name> | exit

local out = os.getenv("PROBE_OUT") or "."
local plan = {}
for item in string.gmatch(os.getenv("PROBE_PLAN") or "", "[^;]+") do
	local frame, action = string.match(item, "^(%d+):(.+)$")
	table.insert(plan, { frame = tonumber(frame), action = action })
end
table.sort(plan, function(a, b) return a.frame < b.frame end)

local HOLD = 4
local held = {}

local function write(path, bytes)
	local f = assert(io.open(path, "wb"))
	f:write(bytes)
	f:close()
end

local function dump(name)
	write(out .. "/" .. name .. "_pal.bin", emu:readRange(0x05000000, 0x400))
	write(out .. "/" .. name .. "_vram.bin", emu:readRange(0x06000000, 0x18000))
	write(out .. "/" .. name .. "_oam.bin", emu:readRange(0x07000000, 0x400))
	write(out .. "/" .. name .. "_io.bin", emu:readRange(0x04000000, 0x60))
	write(out .. "/" .. name .. "_ewram.bin", emu:readRange(0x02000000, 0x40000))
	write(out .. "/" .. name .. "_iwram.bin", emu:readRange(0x03000000, 0x8000))
	console:log("dumped " .. name)
end

local function act(action)
	local kind, arg = string.match(action, "^(%a+):?(.*)$")
	if kind == "shot" then
		emu:screenshot(out .. "/" .. arg .. ".png")
		console:log("shot " .. arg)
	elseif kind == "key" then
		local key = C.GBA_KEY[arg]
		emu:addKey(key)
		held[key] = HOLD
	elseif kind == "dump" then
		dump(arg)
	elseif kind == "exit" then
		os.exit(0)
	end
end

local next_item = 1
callbacks:add("frame", function()
	local frame = emu:currentFrame()
	for key, left in pairs(held) do
		if left <= 1 then
			emu:clearKeys(1 << key)
			held[key] = nil
		else
			held[key] = left - 1
		end
	end
	while next_item <= #plan and plan[next_item].frame <= frame do
		act(plan[next_item].action)
		next_item = next_item + 1
	end
end)
