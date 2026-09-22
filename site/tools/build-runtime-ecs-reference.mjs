import fs from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "../..");
const dataDirectory = path.join(root, "site", "data");
const current = JSON.parse(fs.readFileSync(path.join(dataDirectory, "current.json"), "utf8"));
const snapshot = path.join(dataDirectory, current.snapshot);
const profile = JSON.parse(fs.readFileSync(path.join(snapshot, "profile.json"), "utf8"));
const types = JSON.parse(fs.readFileSync(path.join(snapshot, "types.json"), "utf8"));
const outputDirectory = snapshot;

fs.mkdirSync(outputDirectory, { recursive: true });

const entries = Object.entries(types)
  .filter(([name]) => name.startsWith("keen::ecs::"))
  .sort(([left], [right]) => left.localeCompare(right))
  .map(([name, type]) => {
    const chain = innerTypeChain(type);
    const fields = Object.fromEntries(
      Object.entries(type.fields || {}).map(([field, data]) => [
        field,
        { type: data.type_name, offset: data.data_offset },
      ]),
    );
    return {
      qualified_name: name,
      impact_name: type.impact_name,
      kind: classify(name, type, chain),
      size: type.size,
      alignment: type.alignment,
      inner_type: type.inner_type || null,
      inner_type_chain: chain,
      field_count: Object.keys(fields).length,
      fields,
    };
  });

const summary = entries.reduce((accumulator, entry) => {
  accumulator[entry.kind] = (accumulator[entry.kind] || 0) + 1;
  return accumulator;
}, {});

const output = {
  generated_from: current.snapshot,
  game_version: profile.game_version,
  generated_at: new Date().toISOString(),
  total: entries.length,
  summary,
  entries,
};

fs.writeFileSync(
  path.join(outputDirectory, "runtime-ecs-types.json"),
  JSON.stringify(output, null, 2) + "\n",
);
fs.writeFileSync(path.join(outputDirectory, "runtime-ecs-types.md"), markdown(output));

console.log(
  `Generated ${entries.length} keen::ecs runtime type references: ` +
    Object.entries(summary)
      .sort()
      .map(([kind, count]) => `${kind}=${count}`)
      .join(", "),
);

function innerTypeChain(type) {
  const out = [];
  let next = type.inner_type;
  const seen = new Set();
  while (next && types[next] && !seen.has(next)) {
    seen.add(next);
    out.push(next);
    next = types[next].inner_type;
  }
  return out;
}

function classify(name, type, chain) {
  if (name === "keen::ecs::Component" || chain.includes("keen::ecs::Component")) {
    return "component";
  }
  if (
    name === "keen::ecs::Event" ||
    chain.includes("keen::ecs::Event") ||
    chain.includes("keen::ecs::GameEvent")
  ) {
    return "event";
  }
  if (type.primitive === "Struct") return "runtime-struct";
  return "runtime-type";
}

function markdown(catalog) {
  let text = `# Runtime ECS type reference\n\n`;
  text += `This is a generated filtered view of the public ShroudForge type API, not a second API.\n\n`;
  text += `Generated from \`${catalog.generated_from}\`.\n\n`;
  text += `Game version: \`${catalog.game_version}\`\n\n`;
  text += `| Kind | Count |\n| --- | ---: |\n`;
  for (const [kind, count] of Object.entries(catalog.summary).sort()) {
    text += `| ${kind} | ${count} |\n`;
  }
  text += `| total | ${catalog.total} |\n\n`;
  text += `The types below are already reachable through \`game.types.get(\"keen::ecs::...\")\`.\n`;
  text += `Live runtime access still depends on a verified provider for \`runtime.ecs.query/read/write\`.\n\n`;
  text += `## Important runtime/gameplay types\n\n`;
  text += `| Type | Kind | Size | Fields |\n| --- | --- | ---: | --- |\n`;
  for (const wanted of importantTypes()) {
    const entry = catalog.entries.find((item) => item.qualified_name === `keen::ecs::${wanted}`);
    if (!entry) continue;
    text += `| \`${entry.qualified_name}\` | ${entry.kind} | ${entry.size} | ${Object.keys(entry.fields)
      .map((field) => `\`${field}\``)
      .join(", ")} |\n`;
  }
  return text;
}

function importantTypes() {
  return [
    "EntityId",
    "GameObjectId",
    "CurrentTransform",
    "ClientCursor",
    "NetworkCursor",
    "BuildingPlaceEvent",
    "BuildingTearDownEvent",
    "Flying",
    "DynamicFlying",
    "EnterFlyingStateEvent",
    "StartFlyingEvent",
    "StopFlyingEvent",
    "Stamina",
    "StaminaDepletion",
    "StaminaRecharge",
    "FallDamage",
    "DynamicFallDamage",
    "ItemUsed",
    "Inventory",
    "Crafting",
    "PlayerCraftingAction",
    "Durability",
  ];
}
