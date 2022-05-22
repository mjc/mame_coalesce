local open = io.open

local function read_file(path)
    local file = open(path, "rb") -- r read mode and b binary mode
    if not file then return nil end
    local content = file:read "*a" -- *a or *all reads the whole file
    file:close()
    return content
end

wrk.method = "POST"
wrk.body = read_file("fixtures/Nintendo - Nintendo 64 (BigEndian) (Parent-Clone) (20220127-125048).dat")
wrk.headers["content-type"] = "application/xml"