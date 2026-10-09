-- A browser page's console runs before the shared prelude: what the
-- desktop's engine gives Lua from Rust, written in Lua over the page's
-- hooks. `__pc_ask(id, args_json)` (a promise of the answer's JSON, or of
-- the command's refusal) and `__pc_print(line)` come from the worker.
-- Values cross as JSON with the engine's own rules: a table whose keys are
-- 1 to n, or one made with `array`, is a list, an empty table is the
-- empty list, any other table an object, its keys written as text.

local ARRAY = {}
local DEPTH = 32

function array(...)
  return setmetatable(table.pack(...), ARRAY)
end

local escapes = {
  ['"'] = '\\"', ["\\"] = "\\\\", ["\b"] = "\\b", ["\f"] = "\\f",
  ["\n"] = "\\n", ["\r"] = "\\r", ["\t"] = "\\t",
}

local function quote(s)
  return '"' .. s:gsub('[%c"\\]', function(c)
    return escapes[c] or string.format("\\u%04x", c:byte())
  end) .. '"'
end

local encode

local function list_of(t, n, depth)
  local parts = {}
  for i = 1, n do parts[i] = encode(t[i], depth + 1) end
  return "[" .. table.concat(parts, ",") .. "]"
end

function encode(v, depth)
  depth = depth or 0
  local kind = type(v)
  if v == nil then return "null" end
  if kind == "boolean" then return tostring(v) end
  if kind == "number" then
    if math.type(v) == "integer" then return string.format("%d", v) end
    if v ~= v or v == math.huge or v == -math.huge then return "null" end
    return string.format("%.17g", v)
  end
  if kind == "string" then return quote(v) end
  if kind == "table" and depth < DEPTH then
    if getmetatable(v) == ARRAY then return list_of(v, v.n or #v, depth) end
    local count = 0
    for _ in pairs(v) do count = count + 1 end
    local n = #v
    if count == 0 then return "[]" end
    if n > 0 and count == n then return list_of(v, n, depth) end
    local parts = {}
    for k, item in pairs(v) do
      parts[#parts + 1] = quote(tostring(k)) .. ":" .. encode(item, depth + 1)
    end
    return "{" .. table.concat(parts, ",") .. "}"
  end
  return quote(tostring(v))
end

local function decode(text)
  local pos = 1
  local value

  local function space()
    pos = text:find("[^ \t\r\n]", pos) or #text + 1
  end

  local function fail(what)
    error(string.format("an answer did not read as JSON (%s at %d)", what, pos), 0)
  end

  local function str()
    local out = {}
    pos = pos + 1
    while true do
      local c = text:sub(pos, pos)
      if c == "" then fail("an unended string") end
      if c == '"' then pos = pos + 1; break end
      if c == "\\" then
        local e = text:sub(pos + 1, pos + 1)
        local plain = { b = "\b", f = "\f", n = "\n", r = "\r", t = "\t" }
        if e == "u" then
          local code = tonumber(text:sub(pos + 2, pos + 5), 16)
          pos = pos + 6
          if code >= 0xD800 and code <= 0xDBFF and text:sub(pos, pos + 1) == "\\u" then
            local low = tonumber(text:sub(pos + 2, pos + 5), 16)
            code = 0x10000 + (code - 0xD800) * 0x400 + (low - 0xDC00)
            pos = pos + 6
          end
          out[#out + 1] = utf8.char(code)
        else
          out[#out + 1] = plain[e] or e
          pos = pos + 2
        end
      else
        local run = text:find('["\\]', pos) or #text + 1
        out[#out + 1] = text:sub(pos, run - 1)
        pos = run
      end
    end
    return table.concat(out)
  end

  function value()
    space()
    local c = text:sub(pos, pos)
    if c == "{" then
      local t = {}
      pos = pos + 1
      space()
      if text:sub(pos, pos) == "}" then pos = pos + 1; return t end
      while true do
        space()
        if text:sub(pos, pos) ~= '"' then fail("a key") end
        local k = str()
        space()
        if text:sub(pos, pos) ~= ":" then fail("a colon") end
        pos = pos + 1
        t[k] = value()
        space()
        local sep = text:sub(pos, pos)
        pos = pos + 1
        if sep == "}" then return t end
        if sep ~= "," then fail("a comma") end
      end
    elseif c == "[" then
      local t = {}
      pos = pos + 1
      space()
      if text:sub(pos, pos) == "]" then pos = pos + 1; return t end
      while true do
        t[#t + 1] = value()
        space()
        local sep = text:sub(pos, pos)
        pos = pos + 1
        if sep == "]" then return t end
        if sep ~= "," then fail("a comma") end
      end
    elseif c == '"' then
      return str()
    elseif text:sub(pos, pos + 3) == "true" then
      pos = pos + 4; return true
    elseif text:sub(pos, pos + 4) == "false" then
      pos = pos + 5; return false
    elseif text:sub(pos, pos + 3) == "null" then
      pos = pos + 4; return nil
    end
    local number = text:match("^-?%d+%.?%d*[eE]?[-+]?%d*", pos)
    if not number or number == "" then fail("a value") end
    pos = pos + #number
    if number:find("[.eE]") then return tonumber(number) end
    return math.tointeger(tonumber(number)) or tonumber(number)
  end

  return value()
end

-- A command call: its arguments as JSON to the page, which runs the
-- command where the document is; the script waits for the answer.
function __pc_call(id, args)
  if args ~= nil and type(args) ~= "table" then
    error("a command takes a table of named arguments, like {length = 10}", 2)
  end
  local answer = __pc_ask(id, args == nil and "{}" or encode(args)):await()
  return decode(answer)
end

-- One run: a console line answers its value (shown as `show` writes it),
-- a script what it returns. Both answer the returned values as JSON.
function __pc_run(source, name, as_line)
  local chunk
  if as_line then chunk = load("return " .. source, "=" .. name) end
  if not chunk then
    local err
    chunk, err = load(source, "=" .. name)
    if not chunk then error(err, 0) end
  end
  local results = table.pack(chunk())
  local any = false
  for i = 1, results.n do
    if results[i] ~= nil then any = true end
  end
  if not any then return nil, nil end
  local returned = results.n == 1 and encode(results[1]) or list_of(results, results.n, 0)
  if not as_line then return nil, returned end
  local shown = {}
  for i = 1, results.n do shown[i] = show(results[i]) end
  return table.concat(shown, "\t"), returned
end
