-- Per-frame trace while a PROBE_PLAN runs: the OAM entries listed in
-- TRACE_OAM (x, y, tile, palette) and the BG2 scroll registers, from frame
-- TRACE_FROM on. Plan actions: key:<KEY>, hold:<KEY>x<frames>, shot:<name>, exit.
local plan = {}
for item in string.gmatch(os.getenv("PROBE_PLAN") or "", "[^;]+") do
	local frame, action = string.match(item, "^(%d+):(.+)$")
	table.insert(plan, { frame = tonumber(frame), action = action })
end
table.sort(plan, function(a, b) return a.frame < b.frame end)
local entries = {}
for idx in string.gmatch(os.getenv("TRACE_OAM") or "4", "%d+") do
	table.insert(entries, tonumber(idx))
end
local from = tonumber(os.getenv("TRACE_FROM") or "0")
local out = os.getenv("PROBE_OUT") or "."
local HOLD = 4
local held = {}

local function u16(buf, off)
	return string.byte(buf, off + 1) | (string.byte(buf, off + 2) << 8)
end

local function trace(frame)
	local oam = emu:readRange(0x07000000, 0x400)
	local parts = {}
	for _, i in ipairs(entries) do
		local a0, a1, a2 = u16(oam, i * 8), u16(oam, i * 8 + 2), u16(oam, i * 8 + 4)
		parts[#parts + 1] = string.format("o%d=(%d,%d,t%03x,p%d)", i, a1 & 0x1FF, a0 & 0xFF, a2 & 0x3FF, a2 >> 12)
	end
	local scroll = emu:readRange(0x03004BAE, 8)
	console:log(string.format("TRACE %d %s hofs=%d vofs=%d", frame, table.concat(parts, " "), u16(scroll, 0), u16(scroll, 4)))
end

local next_item = 1
callbacks:add("frame", function()
	local frame = emu:currentFrame()
	for key, left in pairs(held) do
		if left <= 1 then emu:clearKeys(1 << key); held[key] = nil else held[key] = left - 1 end
	end
	while next_item <= #plan and plan[next_item].frame <= frame do
		local kind, arg = string.match(plan[next_item].action, "^(%a+):?(.*)$")
		if kind == "key" then
			local key = C.GBA_KEY[arg]; emu:addKey(key); held[key] = HOLD
		elseif kind == "hold" then
			local name, n = string.match(arg, "^(%a+)x(%d+)$")
			local key = C.GBA_KEY[name]; emu:addKey(key); held[key] = tonumber(n)
		elseif kind == "shot" then
			emu:screenshot(out .. "/" .. arg .. ".png")
		elseif kind == "exit" then
			os.exit(0)
		end
		next_item = next_item + 1
	end
	if frame >= from then trace(frame) end
end)
