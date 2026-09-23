-- Log the code that writes given addresses: PC, registers and frame.
--   PROBE_PLAN   as in mgba_probe.lua (keys/exit), executed by frame
--   WATCH_ADDRS  comma-separated hex addresses to watch for writes
--   WATCH_FROM   frame at which the watchpoints are armed
--   WATCH_MAX    hits to log per address before clearing it

local plan = {}
for item in string.gmatch(os.getenv("PROBE_PLAN") or "", "[^;]+") do
	local frame, action = string.match(item, "^(%d+):(.+)$")
	table.insert(plan, { frame = tonumber(frame), action = action })
end
table.sort(plan, function(a, b) return a.frame < b.frame end)
local addrs = {}
for a in string.gmatch(os.getenv("WATCH_ADDRS") or "", "[^,]+") do
	table.insert(addrs, tonumber(a, 16))
end
local from = tonumber(os.getenv("WATCH_FROM") or "0")
local max_hits = tonumber(os.getenv("WATCH_MAX") or "3")
local watch_type = C.WATCHPOINT_TYPE[os.getenv("WATCH_TYPE") or "WRITE"]

local HOLD = 4
local held = {}
local hits = {}
local ids = {}
local armed = false

local function regs()
	local names = { "r0", "r1", "r2", "r3", "r4", "r5", "r6", "r7", "r8", "r9", "r10", "r11", "r12", "sp", "lr", "pc" }
	local out = {}
	for _, n in ipairs(names) do
		out[#out + 1] = string.format("%s=%08x", n, emu:readRegister(n))
	end
	return table.concat(out, " ")
end

local function arm()
	for _, a in ipairs(addrs) do
		hits[a] = 0
		ids[a] = emu:setWatchpoint(function(info)
			hits[a] = hits[a] + 1
			local desc = ""
			if type(info) == "table" then
				for k, v in pairs(info) do desc = desc .. string.format(" %s=%s", tostring(k), tostring(v)) end
			end
			console:log(string.format("WATCH %08x hit %d frame %d:%s\n   %s", a, hits[a], emu:currentFrame(), desc, regs()))
			if hits[a] >= max_hits then emu:clearBreakpoint(ids[a]) end
		end, a, watch_type)
	end
	armed = true
	console:log("armed " .. #addrs .. " watchpoints")
end

local next_item = 1
callbacks:add("frame", function()
	local frame = emu:currentFrame()
	for key, left in pairs(held) do
		if left <= 1 then emu:clearKeys(1 << key); held[key] = nil else held[key] = left - 1 end
	end
	if not armed and frame >= from then arm() end
	while next_item <= #plan and plan[next_item].frame <= frame do
		local kind, arg = string.match(plan[next_item].action, "^(%a+):?(.*)$")
		if kind == "key" then
			local key = C.GBA_KEY[arg]; emu:addKey(key); held[key] = HOLD
		elseif kind == "hold" then
			local name, n = string.match(arg, "^(%a+)x(%d+)$")
			local key = C.GBA_KEY[name]; emu:addKey(key); held[key] = tonumber(n)
		elseif kind == "exit" then
			os.exit(0)
		end
		next_item = next_item + 1
	end
end)
