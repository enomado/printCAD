-- Links between the guides go to their pages; links into the source tree
-- go to the file on GitHub. `source` is the guide's path in the repository.
local blob = "https://github.com/gilbertorconde/printCAD/blob/master/"
local tree = "https://github.com/gilbertorconde/printCAD/tree/master/"

local function dirname(path)
  return path:match("^(.*)/") or ""
end

local function join(dir, rel)
  local parts = {}
  for part in (dir .. "/" .. rel):gmatch("[^/]+") do
    if part == ".." then
      table.remove(parts)
    elseif part ~= "." then
      table.insert(parts, part)
    end
  end
  return table.concat(parts, "/")
end

local source = ""

function Meta(meta)
  source = pandoc.utils.stringify(meta.source or "")
end

function Link(link)
  local target = link.target
  if target:match("^%a+:") or target:match("^#") then
    return link
  end
  local path, anchor = target:match("^([^#]*)(#?.*)$")
  local repo = join(dirname(source), path)
  -- The design notes stay out of the site: a link to one is its text.
  if repo:match("^docs/rfcs/") then
    return link.content
  end
  -- A guide in docs/ is a page here.
  local page = repo:match("^docs/(.+)%.md$")
  if page then
    local here = source:match("^docs/(.*)$") or ""
    local depth = select(2, here:gsub("/", ""))
    link.target = string.rep("../", depth) .. page .. ".html" .. anchor
  elseif repo:match("^docs/recipes/?$") then
    local here = source:match("^docs/(.*)$") or ""
    local depth = select(2, here:gsub("/", ""))
    link.target = string.rep("../", depth) .. "#recipes"
  elseif path:match("/$") or not path:match("%.[%w]+$") then
    link.target = tree .. repo .. anchor
  else
    link.target = blob .. repo .. anchor
  end
  return link
end

-- The page's first heading is its title, drawn by the template above the
-- body, so the table of contents lists only the sections.
function Pandoc(doc)
  for i, block in ipairs(doc.blocks) do
    if block.t == "Header" and block.level == 1 then
      doc.meta.heading = block.content
      doc.blocks:remove(i)
      break
    end
  end
  return doc
end

return { { Meta = Meta }, { Link = Link }, { Pandoc = Pandoc } }
