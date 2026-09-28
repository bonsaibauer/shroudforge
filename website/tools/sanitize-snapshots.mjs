import fs from "node:fs";
import path from "node:path";

const data = path.resolve(import.meta.dirname, "../data");
for (const entry of fs.readdirSync(data, { withFileTypes: true })) {
  if (!entry.isDirectory()) continue;
  const file = path.join(data, entry.name, "types.json");
  if (!fs.existsSync(file)) continue;
  const types = JSON.parse(fs.readFileSync(file, "utf8"));
  for (const type of Object.values(types)) {
    delete type.name_hash;
    delete type.impact_hash;
    delete type.qualified_hash;
    delete type.internal_hash;
  }
  fs.writeFileSync(file, `${JSON.stringify(types, null, 2)}\n`);
  console.log(`Sanitized public type snapshot: ${entry.name}`);
}
