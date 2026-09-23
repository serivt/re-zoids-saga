-- Dump the m4a driver's PCM buffer every frame: PCM_ADDR (hex) and PCM_SIZE
-- bytes, from frame PCM_FROM to PCM_TO, appended raw to PCM_OUT, one record
-- per frame prefixed by the frame number (u32) and the byte at PCM_COUNTER,
-- followed by PCM_EXTRA_SIZE bytes from PCM_EXTRA (the PSG channel states).
--   PROBE_PLAN   as in mgba_probe.lua (keys/exit)
local plan = {}
for item in string.gmatch(os.getenv("PROBE_PLAN") or "", "[^;]+") do
	local frame, action = string.match(item, "^(%d+):(.+)$")
	table.insert(plan, { frame = tonumber(frame), action = action })
end
table.sort(plan, function(a, b) return a.frame < b.frame end)
local addr = tonumber(os.getenv("PCM_ADDR") or "3006918", 16)
local size = tonumber(os.getenv("PCM_SIZE") or "3168")
local counter = tonumber(os.getenv("PCM_COUNTER") or "3006604", 16)
local extra = tonumber(os.getenv("PCM_EXTRA") or "3007010", 16)
local extra_size = tonumber(os.getenv("PCM_EXTRA_SIZE") or "256")
local from = tonumber(os.getenv("PCM_FROM") or "0")
local to = tonumber(os.getenv("PCM_TO") or "100")
local out = io.open(os.getenv("PCM_OUT") or "pcm.bin", "wb")
local HOLD = 4
local held = {}
local next_item = 1
local function u32(v)
	return string.char(v % 256, math.floor(v / 256) % 256, math.floor(v / 65536) % 256, math.floor(v / 16777216) % 256)
end
callbacks:add("frame", function()
	local frame = emu:currentFrame()
	for key, left in pairs(held) do
		if left <= 1 then emu:clearKeys(1 << key); held[key] = nil else held[key] = left - 1 end
	end
	if frame >= from and frame <= to then
		out:write(u32(frame))
		out:write(string.char(emu:read8(counter)))
		out:write(emu:readRange(addr, size))
		out:write(emu:readRange(extra, extra_size))
		if frame == to then out:close(); console:log("pcm done") end
	end
	while next_item <= #plan and plan[next_item].frame <= frame do
		local kind, arg = string.match(plan[next_item].action, "^(%a+):?(.*)$")
		if kind == "key" then
			local key = C.GBA_KEY[arg]; emu:addKey(key); held[key] = HOLD
		elseif kind == "exit" then
			os.exit(0)
		end
		next_item = next_item + 1
	end
end)
