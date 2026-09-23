-- Steer the player next to an entity and press A: closed-loop on the
-- entity table at IWRAM 0x03004BBC (0x88 bytes per slot, metatile at +0x4E/+0x50).
--   PROBE_PLAN  as in mgba_probe.lua (intro skip), executed by frame
--   TALK_FROM   frame at which steering starts
--   TALK_SLOT   entity slot to approach
--   PROBE_OUT   output directory for screenshots/dumps

local out = os.getenv("PROBE_OUT") or "."
local plan = {}
for item in string.gmatch(os.getenv("PROBE_PLAN") or "", "[^;]+") do
	local frame, action = string.match(item, "^(%d+):(.+)$")
	table.insert(plan, { frame = tonumber(frame), action = action })
end
table.sort(plan, function(a, b) return a.frame < b.frame end)
local from = tonumber(os.getenv("TALK_FROM") or "3840")
local slot = tonumber(os.getenv("TALK_SLOT") or "4")

local held = {}
local function press(name, n)
	local key = C.GBA_KEY[name]
	emu:addKey(key)
	held[key] = n
end

local function write(path, bytes)
	local f = assert(io.open(path, "wb")); f:write(bytes); f:close()
end

local function dump(name)
	write(out .. "/" .. name .. "_iwram.bin", emu:readRange(0x03000000, 0x8000))
	write(out .. "/" .. name .. "_ewram.bin", emu:readRange(0x02000000, 0x40000))
	write(out .. "/" .. name .. "_vram.bin", emu:readRange(0x06000000, 0x18000))
	write(out .. "/" .. name .. "_pal.bin", emu:readRange(0x05000000, 0x400))
	write(out .. "/" .. name .. "_oam.bin", emu:readRange(0x07000000, 0x400))
	emu:screenshot(out .. "/" .. name .. ".png")
	console:log("captured " .. name)
end

local function cell(s)
	local b = emu:readRange(0x03004BBC + s * 0x88, 0x88)
	local function u16(o) return string.byte(b, o + 1) | (string.byte(b, o + 2) << 8) end
	return u16(0x4E), u16(0x50), u16(0x4A), u16(0x30)
end

local next_item = 1
local phase = "walk"
local talked_at = nil
local shots = 0
callbacks:add("frame", function()
	local frame = emu:currentFrame()
	for key, left in pairs(held) do
		if left <= 1 then emu:clearKeys(1 << key); held[key] = nil else held[key] = left - 1 end
	end
	while next_item <= #plan and plan[next_item].frame <= frame do
		local kind, arg = string.match(plan[next_item].action, "^(%a+):?(.*)$")
		if kind == "key" then press(arg, 4)
		elseif kind == "hold" then local name, n = string.match(arg, "^(%a+)x(%d+)$"); press(name, tonumber(n))
		elseif kind == "exit" then os.exit(0) end
		next_item = next_item + 1
	end
	if frame < from or next(held) ~= nil then return end
	local pc, pr, pstate, panim = cell(1)
	local nc, nr = cell(slot)
	if pstate ~= 0 and phase ~= "talk" then return end
	local dc, dr = nc - pc, nr - pr
	local adjacent = math.abs(dc) + math.abs(dr) == 1
	local dir, index
	if adjacent then
		if dc == 1 then dir, index = "RIGHT", 3 elseif dc == -1 then dir, index = "LEFT", 2 elseif dr == 1 then dir, index = "DOWN", 1 else dir, index = "UP", 0 end
	end
	if phase == "walk" then
		if adjacent then
			press(dir, 2)
			phase = "turn"
		elseif math.abs(dc) > math.abs(dr) then
			press(dc > 0 and "RIGHT" or "LEFT", 14)
		else
			press(dr > 0 and "DOWN" or "UP", 14)
		end
	elseif phase == "turn" then
		if not adjacent then phase = "walk"; return end
		if panim ~= index then press(dir, 2); return end
		console:log(string.format("frame %d player (%d,%d) anim %d npc (%d,%d): pressing A", frame, pc, pr, panim, nc, nr))
		press("A", 4)
		phase = "talk"
		talked_at = frame
		dump("talk_a0")
	elseif phase == "talk" then
		local t = frame - talked_at
		if t <= 110 then
			local map = emu:readRange(0x06000000, 0x800)
			local n = 0
			for i = 0, 0x7FE, 2 do
				local e = (string.byte(map, i + 1) | (string.byte(map, i + 2) << 8)) & 0x3FF
				if e >= 384 or e == 348 then n = n + (e == 348 and 1000 or 1) end
			end
			console:log(string.format("TW %d %d", t, n))
		end
		if t == 20 or t == 60 or t == 120 then dump("talk_a" .. t) end
		if t == 121 or t == 241 then press("A", 4); dump("talk_a" .. t) end
		if t == 200 or t == 300 or t == 360 then dump("talk_a" .. t) end
		if t >= 400 then os.exit(0) end
	end
end)
