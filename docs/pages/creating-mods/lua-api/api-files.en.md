---
{
  "id": "api-files",
  "locale": "en",
  "title": "Read files and save exports",
  "summary": "Package files and exported files use different locations and permissions. Choose the matching API surface.",
  "navigation": {
    "path": [
      "Lua API guides",
      "Feature guides"
    ]
  },
  "apiPrefixes": [
    "io.",
    "buffer.",
    "Buffer:",
    "game.assets.",
    "shroudforge.contents."
  ]
}
---

# Read files and save exports

The global `io` API separates bundled package files from player exports. A mod should not assume that these areas use the same path or write permission.

## Capability

Export functions require `export` in `mod.json`. Check the API result before reporting success. Use the [API reference](#api) to look up exact functions and arguments.

## Typical flow

1. Choose a relative export path.
2. Create bytes with the matching buffer or serialization function.
3. Call the export function.
4. Check its result and log a useful success or failure message.

The loader keeps paths inside the mod's permitted area. Export is not unrestricted access to arbitrary files on the computer.

## Read large game contents without waiting

`game.assets` reads a content's bytes on the mods' loop, which every mod waits for. For large contents, such as the levels of a texture, use `shroudforge.contents.read`: it reads the requested byte range on a background thread and returns a job that the mod asks at its next updates.

```lua
local job = shroudforge.contents.read({hash = texture.content.hash, start = 0, count = 65536})

-- At a later update:
local result = job:result()
if result and result.data then
  -- result.data holds the bytes, result.seconds the read time.
elseif result then
  shroudforge.log.warn(result.error)
end
```

`hash` is a content hash as a resource holds it. `start` and `count` select a byte range, the whole content by default. Only contents of the game files can be read, not those a mod created.

Each request is limited to 32 MiB. Split a larger content into multiple ranges. At most four reads may be outstanding per runtime. A completed job still uses one slot until its result is read or the job is released. Invalid ranges return an error instead of being shortened. These are initial safety limits for this API. They prevent unbounded threads and allocations while supporting Architect Lines' current texture reads. The values can be raised after measuring the mod's actual range sizes and memory use.
