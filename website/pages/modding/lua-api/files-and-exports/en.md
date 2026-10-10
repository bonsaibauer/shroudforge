# Read files and save exports

The `shroudforge.io` API separates bundled package files from player exports. A mod should not assume that these areas use the same path or write permission.

## Capability

Export functions require `export` in `mod.json`. Check the API result before reporting success. Use the [API reference](#api) to look up exact functions and arguments.

## Typical flow

1. Choose a relative export path.
2. Create bytes with the matching buffer or serialization function.
3. Call the export function.
4. Check its result and log a useful success or failure message.

The loader keeps paths inside the mod's permitted area. Export is not unrestricted access to arbitrary files on the computer.
