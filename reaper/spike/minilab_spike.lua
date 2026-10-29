-- MiniLab 3 spike: a background ReaScript that proves the loop between Reaper and the device.
-- Throwaway code: it answers "does this work?", the real version is the Rust extension.
--
-- Out: connect; screen = selected track name + position/volume, play/record/arm icons; pads bank A = track
--      colours (selected bright); Shift-layer transport LEDs from the play state; knob realign + parameter popups.
-- In:  Shift + encoder = select track, Shift + encoder click = arm, Shift + pads = transport,
--      fader 1 = volume, knobs 1-8 = parameters of the selected track's first FX, pads bank A = select track 1-8.
--      Program/bank change from the device = connect again and redraw.
--
-- Protocol: docs/minilab3-protocol.md. Log: .scratch/spike.log (when the repo path below exists).

local LOG_PATH = "/home/can/Projects/repl/minilab-reaper-integration/.scratch/spike.log"
local logf = io.open(LOG_PATH, "w")
local function log(...)
  if not logf then return end
  logf:write(string.format("%.3f ", reaper.time_precise()), table.concat({...}, " "), "\n")
  logf:flush()
end

local function hex(s) return (s:gsub(".", function(c) return string.format("%02X ", c:byte()) end)) end
local function bytes(t) local s = {} for i, b in ipairs(t) do s[i] = string.char(b) end return table.concat(s) end
local function ascii(text)
  local t = {}
  for c in text:gmatch(".") do local b = c:byte(); t[#t+1] = (b >= 0x20 and b < 0x7F) and b or 0x3F end
  return t
end
local function concat(...)
  local out = {}
  for _, t in ipairs({...}) do for _, b in ipairs(t) do out[#out+1] = b end end
  return out
end

-- Device ports ---------------------------------------------------------------------------------------------------

local function find_device(count_fn, name_fn)
  for i = 0, count_fn() - 1 do
    local ok, name = name_fn(i, "")
    if ok and name:find("Minilab3: MIDI") then return i end
  end
end
local OUT = find_device(reaper.GetNumMIDIOutputs, reaper.GetMIDIOutputName)
local IN = find_device(reaper.GetNumMIDIInputs, reaper.GetMIDIInputName)
if not OUT or not IN then
  reaper.ShowMessageBox("MiniLab 3 not found. Enable 'Minilab3: MIDI' input and output in Preferences > MIDI Devices.",
    "MiniLab spike", 0)
  return
end
log("ports in", IN, "out", OUT)

-- Messages ---------------------------------------------------------------------------------------------------------

local HEADER = {0xF0, 0x00, 0x20, 0x6B, 0x7F, 0x42}
local function sysex(body) return concat(HEADER, body, {0xF7}) end

-- Keyed, paced send. A message is queued only when it differs from the last one for its key, and a newer message
-- replaces a queued one with the same key. flush() sends at most PER_TICK per defer cycle (~30 ms): bursts of ~200
-- messages with no gap lose some on the device (docs/minilab3-protocol.md, pacing).
local PER_TICK = 8
local last_sent, queue, queued = {}, {}, {}
local function send(key, msg)
  local s = bytes(msg)
  if last_sent[key] == s and not queued[key] then return end
  if not queued[key] then queue[#queue+1] = key end
  queued[key] = s
end
local function flush()
  local n = 0
  while #queue > 0 and n < PER_TICK do
    local key = table.remove(queue, 1)
    local s = queued[key]
    queued[key] = nil
    if last_sent[key] ~= s then
      reaper.SendMIDIMessageToHardware(OUT, s)
      last_sent[key] = s
      log("OUT", key, hex(s))
      n = n + 1
    end
  end
end
local function forget_sent() last_sent = {} end

local function connect() send("connect", sysex({0x02, 0x00, 0x40, 0x6A, 0x21})) end
local function disconnect() reaper.SendMIDIMessageToHardware(OUT, bytes(sysex({0x02, 0x00, 0x40, 0x6A, 0x20}))) end

local ICON = {none = 0, arp = 1, play = 2, record = 3, arm = 4}
local function screen(line1, line2, transport, armed)
  send("screen", sysex(concat({0x04, 0x02, 0x60, 0x1F, 0x07, 0x01, ICON[transport], armed and ICON.arm or 0, 0, 0x00, 0x01},
    ascii(line1), {0x00, 0x02}, ascii(line2), {0x00})))
end
local function popup(cc, name, value)
  send("popup" .. cc, sysex(concat({0x04, 0x02, 0x60, 0x1F, 0x08, 0x02, cc, 0x00, 0x01}, ascii(name), {0x00, 0x02},
    ascii(value), {0x00})))
end
local function led(id, r, g, b)
  send("led" .. id, sysex({0x02, 0x02, 0x16, id, r // 2, g // 2, b // 2}))  -- r, g, b in 0-255
end
local function realign(knob, value) send("knob" .. knob, sysex({0x21, 0x10, 0x00, 7 + knob, 0x00, value})) end

-- Reaper state -----------------------------------------------------------------------------------------------------

local KNOB_CC = {86, 87, 89, 90, 110, 111, 116, 117}
local KNOB_INDEX = {} for i, cc in ipairs(KNOB_CC) do KNOB_INDEX[cc] = i - 1 end
local PAD_A = 0x34
local TRANSPORT_LED = {loop = 0x57, stop = 0x58, play = 0x59, record = 0x5A, tap = 0x5B}

local function selected_track()
  return reaper.GetSelectedTrack(0, 0) or reaper.GetTrack(0, 0)
end

local function track_rgb(tr)
  local c = reaper.GetTrackColor(tr)
  if c == 0 then return 128, 128, 128 end
  return reaper.ColorFromNative(c)
end

local function db(vol) return vol < 0.0000001 and "-inf" or string.format("%.1f dB", 20 * math.log(vol, 10)) end

local function draw()
  local tr = selected_track()
  local state = reaper.GetPlayState()  -- &1 playing, &2 paused, &4 recording
  local transport = (state & 4 ~= 0) and "record" or (state & 1 ~= 0) and "play" or "none"
  if tr then
    local _, name = reaper.GetTrackName(tr)
    local n = math.floor(reaper.GetMediaTrackInfo_Value(tr, "IP_TRACKNUMBER"))
    local armed = reaper.GetMediaTrackInfo_Value(tr, "I_RECARM") == 1
    screen(name, string.format("%d/%d  %s", n, reaper.CountTracks(0), db(reaper.GetMediaTrackInfo_Value(tr, "D_VOL"))),
      transport, armed)
  else
    screen("No tracks", "", transport, false)
  end

  -- Pads bank A: tracks 1-8 in their colours, the selected one bright, the rest dim.
  for i = 0, 7 do
    local t = reaper.GetTrack(0, i)
    if t then
      local r, g, b = track_rgb(t)
      local k = (t == tr) and 1 or 0.15
      led(PAD_A + i, math.floor(r * k), math.floor(g * k), math.floor(b * k))
    else
      led(PAD_A + i, 0, 0, 0)
    end
  end

  -- Shift-layer transport LEDs (Ableton's colours: full when on, about 16% when off).
  local function onoff(on, r, g, b) local k = on and 1 or 0.16 return math.floor(r * k), math.floor(g * k), math.floor(b * k) end
  led(TRANSPORT_LED.loop, onoff(reaper.GetSetRepeat(-1) == 1, 254, 100, 0))
  led(TRANSPORT_LED.stop, onoff(state == 0, 254, 254, 254))
  led(TRANSPORT_LED.play, onoff(state & 1 ~= 0, 0, 254, 0))
  led(TRANSPORT_LED.record, onoff(state & 4 ~= 0, 254, 0, 0))
  led(TRANSPORT_LED.tap, onoff(false, 254, 254, 254))
end

-- Knobs on the selected track's first FX ---------------------------------------------------------------------------

local knob_track
local function knob_target(tr, knob)
  if not tr or reaper.TrackFX_GetCount(tr) == 0 then return nil end
  if knob >= reaper.TrackFX_GetNumParams(tr, 0) then return nil end
  return 0, knob
end

local function param_text(tr, fx, p)
  local _, pname = reaper.TrackFX_GetParamName(tr, fx, p, "")
  local _, ptext = reaper.TrackFX_GetFormattedParamValue(tr, fx, p, "")
  return pname, ptext
end

-- When the selected track changes: move the knobs to its values, preload the popups (name + value per knob).
local function bind_knobs(tr)
  knob_track = tr
  for k = 0, 7 do
    local fx, p = knob_target(tr, k)
    if fx then
      realign(k, math.floor(reaper.TrackFX_GetParamNormalized(tr, fx, p) * 127 + 0.5))
      popup(KNOB_CC[k + 1], param_text(tr, fx, p))
    else
      send("popup" .. KNOB_CC[k + 1], sysex({0x04, 0x02, 0x60, 0x1F, 0x09, 0x02, KNOB_CC[k + 1], 0x00}))  -- blank
    end
  end
end

-- Input --------------------------------------------------------------------------------------------------------

local ACTION = {[105] = 1068, [106] = 1016, [107] = 1007, [108] = 1013, [109] = 1134}  -- loop, stop, play, record, tap

local function select_track_offset(step)
  local tr = selected_track()
  if not tr then return end
  local n = math.floor(reaper.GetMediaTrackInfo_Value(tr, "IP_TRACKNUMBER")) - 1 + step
  n = math.max(0, math.min(reaper.CountTracks(0) - 1, n))
  local t = reaper.GetTrack(0, n)
  reaper.SetOnlyTrackSelected(t)
  reaper.Main_OnCommand(40913, 0)  -- Track: Vertical scroll selected tracks into view
end

local device_status = {}  -- "62" (program), "63" (pad bank) -> last value seen
local function on_message(msg)
  local s, d1, d2 = msg:byte(1, 3)
  if s == 0xF0 then
    -- Status from the device: program (62) or pad bank (63). The device also sends the bank status in reply to
    -- connect, so act only when a value really changes; reacting to every one loops forever.
    local body = hex(msg)
    local what, value = body:match("^F0 00 20 6B 7F 42 02 00 40 (6[23]) (%x%x)")
    if what and device_status[what] ~= value then
      local first = device_status[what] == nil
      device_status[what] = value
      log("device status", what, value)
      if not first then forget_sent(); connect(); bind_knobs(selected_track()) end
    end
    return
  end
  local kind, ch = s & 0xF0, s & 0x0F
  if kind == 0xB0 then
    if d1 == 29 and d2 ~= 64 then                       -- Shift + encoder turn
      select_track_offset(d2 > 64 and 1 or -1)
    elseif d1 == 119 and d2 > 0 then                    -- Shift + encoder click
      local tr = selected_track()
      if tr then reaper.SetMediaTrackInfo_Value(tr, "I_RECARM", 1 - reaper.GetMediaTrackInfo_Value(tr, "I_RECARM")) end
    elseif ACTION[d1] and d2 > 0 then                   -- Shift + transport pads
      reaper.Main_OnCommand(ACTION[d1], 0)
    elseif d1 == 14 then                                -- fader 1: volume, 0..127 -> -inf..+6 dB on Reaper's fader curve
      local tr = selected_track()
      if tr then
        reaper.SetMediaTrackInfo_Value(tr, "D_VOL", reaper.SLIDER2DB and 10 ^ (reaper.SLIDER2DB(d2 / 127 * 1000) / 20) or d2 / 127 * 2)
      end
    elseif KNOB_INDEX[d1] then                          -- knobs 1-8
      local tr = selected_track()
      local fx, p = knob_target(tr, KNOB_INDEX[d1])
      if fx then
        reaper.TrackFX_SetParamNormalized(tr, fx, p, d2 / 127)
        popup(d1, param_text(tr, fx, p))
      end
    end
  elseif kind == 0x90 and ch == 9 and d2 > 0 and d1 >= 36 and d1 <= 43 then  -- pads bank A: select track
    local t = reaper.GetTrack(0, d1 - 36)
    if t then reaper.SetOnlyTrackSelected(t) end
  end
end

local last_idx = reaper.MIDI_GetRecentInputEvent(0)
local function read_input()
  local i, newest, batch = 0, last_idx, {}
  while true do
    local idx, buf, _, dev = reaper.MIDI_GetRecentInputEvent(i)
    if idx == 0 or idx <= last_idx then break end
    if idx > newest then newest = idx end
    if dev == IN then table.insert(batch, 1, buf) end  -- the list is newest first; handle oldest first
    i = i + 1
  end
  last_idx = newest
  for _, m in ipairs(batch) do log("IN", hex(m)); on_message(m) end
end

-- Main loop --------------------------------------------------------------------------------------------------------

-- reaper/spike/stop.lua sets this flag; running the script twice would open Reaper's "already running" dialog.
reaper.DeleteExtState("minilab_spike", "stop", false)
connect()
bind_knobs(selected_track())
local function loop()
  if reaper.GetExtState("minilab_spike", "stop") == "1" then return end  -- atexit runs after this
  read_input()
  local tr = selected_track()
  if tr ~= knob_track then bind_knobs(tr) end
  draw()
  flush()
  reaper.defer(loop)
end
reaper.atexit(function() log("exit"); disconnect(); if logf then logf:close() end end)
log("started")
loop()
