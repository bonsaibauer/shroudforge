import fs from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "../..");
const definitions = path.join(root, "src", "loader", "api", "src");
const dataDirectory = path.join(root, "website", "data");
const schemasDirectory = path.join(root, "website", "schemas");
const output = path.join(dataDirectory, "api.json");
const version = fs.readFileSync(path.join(root, "VERSION"), "utf8").trim();
fs.mkdirSync(schemasDirectory, { recursive: true });
for (const name of ["manifest.schema.json", "extended.mod.schema.json"]) {
  fs.copyFileSync(
    path.join(root, "src", "loader", "package", "src", "registry", name),
    path.join(schemasDirectory, name),
  );
}
const files = walk(definitions).filter(file => file.endsWith(".lua")).sort();
const symbols = [];
const classDescriptions = new Map();
const classAliases = {
  AssetManager: "game.assets",
  BuildingInput: "runtime.world.building.Input",
  BufferFactory: "buffer",
  Game: "game",
  GuidHelper: "game.guid",
  Hasher: "hasher",
  IO: "io",
  RuntimeApi: "runtime",
  RuntimeEcsApi: "runtime.ecs",
  RuntimeLifecycle: "runtime.lifecycle",
  RuntimeFeatureStatus: "runtime.status",
  EmlLoader: "loader",
  EmlLoaderFeatures: "loader.features",
  EmlRuntimeFeatures: "loader.features.runtime",
  EmlLoaderRuntime: "loader.runtime",
  ShroudForgeApi: "shroudforge",
  ShroudForge: "shroudforge",
  ShroudForgeLogApi: "shroudforge.log",
  ShroudForgeNotification: "shroudforge.notifications.Notice",
  ShroudForgeNotifications: "shroudforge.notifications",
  ShroudForgeSettings: "shroudforge.settings",
  ShroudForgeUi: "shroudforge.ui",
  ShroudForgeInput: "shroudforge.input",
  TypeRegistry: "game.types",
};

const snapshots = fs.readdirSync(dataDirectory, { withFileTypes: true })
  .filter(entry => entry.isDirectory() && fs.existsSync(path.join(dataDirectory, entry.name, "profile.json")))
  .map(entry => {
    const profile = JSON.parse(fs.readFileSync(path.join(dataDirectory, entry.name, "profile.json"), "utf8"));
    return { name: entry.name, captured: Date.parse(profile.game_version.split("|")[2]) || 0 };
  })
  .sort((left, right) => right.captured - left.captured);
if (!snapshots.length) throw new Error("website/data contains no game snapshot");
fs.writeFileSync(path.join(dataDirectory, "current.json"), JSON.stringify({ snapshot: snapshots[0].name }, null, 2) + "\n");

for (const file of files) {
  const relative = path.relative(definitions, file).replaceAll("\\", "/");
  const lines = fs.readFileSync(file, "utf8").split(/\r?\n/);
  let documentation = [];
  let currentClass = "";
  for (const line of lines) {
    const doc = line.match(/^---\s?(.*)$/);
    if (doc) {
      const value = doc[1].trim();
      if (/^@alias\b/.test(value)) {
        documentation = [];
        continue;
      }
      const classMatch = value.match(/^@class\s+([^\s:]+)/);
      if (classMatch) {
        currentClass = classMatch[1];
        const description = documentation.filter(text => text && !text.startsWith("@")).join(" ");
        if (description) classDescriptions.set(currentClass, description);
      }
      const fieldMatch = value.match(/^@field\s+(\S+)\s+(\S+\([^)]*\)(?::\S+)?\??|[^\s]+)(?:\s+(?:--\s*)?(.*))?/);
      if (fieldMatch && currentClass) {
        const owner = publicClassName(currentClass);
        const name = `${owner}.${fieldMatch[1]}`;
        symbols.push({
          kind: "field",
          namespace: namespaceOf(name),
          name,
          signature: `${name}: ${fieldMatch[2]}`,
          params: [],
          returns: [fieldMatch[2]],
          description: fieldMatch[3] || "",
          owner: currentClass,
          owner_description: classDescriptions.get(currentClass) || "",
          source: relative,
        });
      }
      documentation.push(value);
      continue;
    }
    const functionMatch = line.match(/^function\s+([^\s(]+)\(([^)]*)\)\s*end/);
    if (functionMatch) {
      const name = publicName(functionMatch[1]);
      const params = documentation.flatMap(value => {
        const match = value.match(/^@param\s+(\S+)\s+(\S+\([^)]*\)|[^\s]+)(?:\s+(?:--\s*)?(.*))?/);
        return match ? [{ name: match[1], type: match[2], description: match[3] || "" }] : [];
      });
      const returnDetails = documentation.flatMap(parseReturnAnnotation);
      const returns = returnDetails.map(value => value.type);
      const sections = parseDocumentationSections(documentation);
      symbols.push({
        kind: "function",
        namespace: namespaceOf(name),
        name,
        signature: `${name}(${functionMatch[2]})`,
        params,
        returns,
        returnDetails,
        description: sections.description,
        errors: sections.errors,
        example: sections.example,
        source: relative,
      });
      documentation = [];
      continue;
    }
    documentation = line.trim() ? [] : documentation;
  }
}

for (const symbol of symbols) {
  if (symbol.source === "eml/v1/definitions/integer.lua") addIntegerHelp(symbol);
  addMissingParameterHelp(symbol);
}

fs.writeFileSync(output, JSON.stringify({ version, symbols }, null, 2) + "\n");
console.log(`Generated ${symbols.length} implemented Lua symbols for ShroudForge API ${version}.`);

function parseReturnAnnotation(value) {
  if (!value.startsWith("@return ")) return [];
  let body = value.slice("@return ".length).trim();
  let inlineDescription = "";
  const comment = body.match(/^(.*?)\s+--\s*(.*)$/);
  if (comment) {
    body = comment[1].trim();
    inlineDescription = comment[2].trim();
  }

  const tokens = body.split(/\s+/).filter(Boolean);
  if (!tokens.length) return [];
  if (/^[^<>{]+,\s*[^<>{]+$/.test(body)) {
    return body.split(/\s*,\s*/).map((type, index) => ({
      type,
      name: "",
      description: index === 0 ? inlineDescription : "",
    }));
  }
  let type = tokens.shift();
  while (tokens[0] === "|") type += ` ${tokens.shift()} ${tokens.shift() || ""}`;
  const types = type.split(/\s*,\s*/).filter(Boolean);
  const looksLikeName = tokens[0] && /^[A-Za-z_][\w]*$/.test(tokens[0]);
  const name = looksLikeName ? tokens.shift() : "";
  const description = [inlineDescription, ...tokens].filter(Boolean).join(" ");
  return types.map((resultType, index) => ({
    type: resultType,
    name: index === 0 ? name : "",
    description: index === 0 ? description : "",
  }));
}

function parseDocumentationSections(documentation) {
  const lines = documentation.filter(value => !value.startsWith("@"));
  const descriptionLines = [];
  for (const line of lines) {
    if (!line) {
      if (descriptionLines.length) break;
      continue;
    }
    if (/^#{1,6}\s/.test(line) || /^```/.test(line)) break;
    descriptionLines.push(line);
  }

  const exampleHeading = lines.findIndex(line => /^#{1,6}\s*Example\b/i.test(line));
  let example = "";
  if (exampleHeading >= 0) {
    const fence = lines.findIndex((line, index) => index > exampleHeading && /^```(?:lua)?\s*$/.test(line));
    if (fence >= 0) {
      const end = lines.findIndex((line, index) => index > fence && /^```\s*$/.test(line));
      if (end > fence) example = lines.slice(fence + 1, end).join("\n").trim();
    }
  }

  const errorHeading = lines.findIndex(line => /^#{1,6}\s*Errors?\b/i.test(line));
  let errors = "";
  if (errorHeading >= 0) {
    const errorLines = [];
    let inCodeFence = false;
    for (const line of lines.slice(errorHeading + 1)) {
      if (/^```/.test(line)) {
        inCodeFence = !inCodeFence;
        continue;
      }
      if (!inCodeFence && /^#{1,6}\s/.test(line)) break;
      if (!inCodeFence && line) errorLines.push(line.replace(/^\s*-\s*/, ""));
    }
    errors = errorLines.join(" ").trim();
  }
  return { description: descriptionLines.join(" ").trim(), errors, example };
}

function addIntegerHelp(symbol) {
  const parts = symbol.name.split(".");
  const integerType = parts.at(-2);
  const operation = parts.at(-1);
  const bits = Number(integerType.slice(1));
  const limits = {
    u8: "0 to 255", u16: "0 to 65,535", u32: "0 to 4,294,967,295",
    u64: "0 to 18,446,744,073,709,551,615", i8: "-128 to 127",
    i16: "-32,768 to 32,767", i32: "-2,147,483,648 to 2,147,483,647",
    i64: "-9,223,372,036,854,775,808 to 9,223,372,036,854,775,807",
  };
  const limitsDe = {
    u8: "0 bis 255", u16: "0 bis 65.535", u32: "0 bis 4.294.967.295",
    u64: "0 bis 18.446.744.073.709.551.615", i8: "-128 bis 127",
    i16: "-32.768 bis 32.767", i32: "-2.147.483.648 bis 2.147.483.647",
    i64: "-9.223.372.036.854.775.808 bis 9.223.372.036.854.775.807",
  };
  const range = limits[integerType] || "the range of this integer type";
  const rangeDe = limitsDe[integerType] || "den Wertebereich dieses Ganzzahltyps";
  const descriptions = {
    MIN: [`Smallest value representable by ${integerType}.`, `Kleinster darstellbarer Wert des Typs ${integerType}.`],
    MAX: [`Largest value representable by ${integerType}.`, `Größter darstellbarer Wert des Typs ${integerType}.`],
    BITS: [`Number of bits used by ${integerType}, which is ${bits}.`, `Anzahl der Bits von ${integerType}, also ${bits}.`],
    parse: [`Parses a decimal string as ${integerType}. Returns nil when the text is invalid or outside ${range}. The declared radix argument is currently ignored.`, `Liest eine Dezimalzahl als ${integerType}. Bei ungültigem Text oder einem Wert außerhalb von ${rangeDe} kommt nil zurück. Das deklarierte Argument radix wird derzeit ignoriert.`],
    truncate: [`Converts a number to ${integerType} with the integer cast rules. Use is_valid first when out-of-range input must be rejected.`, `Wandelt eine Zahl nach den Cast-Regeln in ${integerType} um. Prüfe vorher mit is_valid, wenn Werte außerhalb des Bereichs abgelehnt werden sollen.`],
    clamp: [`Limits the input to the ${integerType} range ${range}.`, `Begrenzt die Eingabe auf den Wertebereich ${rangeDe} von ${integerType}.`],
    is_valid: [`Checks whether the input can be represented by ${integerType}. Floating-point input must also be a whole number.`, `Prüft, ob die Eingabe als ${integerType} darstellbar ist. Eine Fließkommazahl muss zusätzlich ganzzahlig sein.`],
    to_string: [`Converts the input to ${integerType} and returns its decimal text.`, `Wandelt die Eingabe in ${integerType} um und gibt sie als Dezimaltext zurück.`],
    count_ones: [`Counts the set bits in the ${bits}-bit representation.`, `Zählt die gesetzten Bits in der ${bits}-Bit-Darstellung.`],
    count_zeros: [`Counts the unset bits in the ${bits}-bit representation.`, `Zählt die nicht gesetzten Bits in der ${bits}-Bit-Darstellung.`],
    leading_zeros: [`Counts zero bits before the first set bit.`, `Zählt die Null-Bits vor dem ersten gesetzten Bit.`],
    trailing_zeros: [`Counts zero bits after the last set bit.`, `Zählt die Null-Bits nach dem letzten gesetzten Bit.`],
    leading_ones: [`Counts one bits before the first zero bit.`, `Zählt die Eins-Bits vor dem ersten Null-Bit.`],
    trailing_ones: [`Counts one bits after the last zero bit.`, `Zählt die Eins-Bits nach dem letzten Null-Bit.`],
    rotate_left: [`Rotates the bits left by count positions. Bits shifted out on one side re-enter on the other.`, `Rotiert die Bits um count Stellen nach links. Herausgeschobene Bits kommen auf der anderen Seite wieder hinein.`],
    rotate_right: [`Rotates the bits right by count positions. Bits shifted out on one side re-enter on the other.`, `Rotiert die Bits um count Stellen nach rechts. Herausgeschobene Bits kommen auf der anderen Seite wieder hinein.`],
    swap_bytes: [`Reverses the order of the bytes in the integer.`, `Dreht die Reihenfolge der Bytes in der Ganzzahl um.`],
    reverse_bits: [`Reverses the order of all ${bits} bits.`, `Dreht die Reihenfolge aller ${bits} Bits um.`],
    from_be: [`Converts an integer read in big-endian byte order to the host byte order.`, `Wandelt eine im Big-Endian-Format gelesene Ganzzahl in die Byte-Reihenfolge des Rechners um.`],
    from_le: [`Converts an integer read in little-endian byte order to the host byte order.`, `Wandelt eine im Little-Endian-Format gelesene Ganzzahl in die Byte-Reihenfolge des Rechners um.`],
    to_be: [`Converts the integer to big-endian byte order.`, `Wandelt die Ganzzahl in Big-Endian-Byte-Reihenfolge um.`],
    to_le: [`Converts the integer to little-endian byte order.`, `Wandelt die Ganzzahl in Little-Endian-Byte-Reihenfolge um.`],
    bit_and: [`Combines corresponding bits with AND. A result bit is set only when both input bits are set.`, `Verknüpft die Bits mit AND. Ein Ergebnisbit ist nur gesetzt, wenn beide Eingabebits gesetzt sind.`],
    bit_or: [`Combines corresponding bits with OR. A result bit is set when either input bit is set.`, `Verknüpft die Bits mit OR. Ein Ergebnisbit ist gesetzt, wenn mindestens eines der Eingabebits gesetzt ist.`],
    bit_xor: [`Combines corresponding bits with XOR. A result bit is set when the input bits differ.`, `Verknüpft die Bits mit XOR. Ein Ergebnisbit ist gesetzt, wenn sich die Eingabebits unterscheiden.`],
    bit_not: [`Flips every bit in the ${bits}-bit representation.`, `Kehrt jedes Bit in der ${bits}-Bit-Darstellung um.`],
  };

  if (["MIN", "MAX", "BITS"].includes(operation)) {
    symbol.description = descriptions[operation][0];
    symbol.description_de = descriptions[operation][1];
    return;
  }

  const arithmetic = operation.match(/^(checked|saturating|wrapping|overflowing)?_?(add|sub|mul|div|rem|neg|shl|shr|pow)$/);
  if (descriptions[operation]) {
    [symbol.description, symbol.description_de] = descriptions[operation];
  } else if (arithmetic) {
    const [, mode = "wrapping", op] = arithmetic;
    const verb = {
      add: ["Adds", "Addiert"], sub: ["Subtracts", "Subtrahiert"], mul: ["Multiplies", "Multipliziert"],
      div: ["Divides", "Dividiert"], rem: ["Returns the remainder of dividing", "Gibt den Rest der Division von"],
      neg: ["Negates", "Negiert"], shl: ["Shifts the bits left", "Verschiebt die Bits nach links"],
      shr: ["Shifts the bits right", "Verschiebt die Bits nach rechts"], pow: ["Raises the value to the exponent", "Berechnet die Potenz mit dem Wert als Basis"],
    }[op];
    const modeText = {
      checked: ["Returns nil if the operation overflows or is invalid.", "Gibt nil zurück, wenn die Operation überläuft oder ungültig ist."],
      saturating: [`Clamps overflow to the nearest limit of ${range}.`, `Begrenzt einen Überlauf auf den nächstliegenden Grenzwert des Wertebereichs ${rangeDe}.`],
      wrapping: [`Wraps overflow around the ${integerType} range ${range}.`, `Lässt einen Überlauf im Wertebereich ${rangeDe} von ${integerType} umlaufen.`],
      overflowing: [`Returns the wrapped result and a second boolean that reports overflow.`, `Gibt das umlaufende Ergebnis und als zweiten Rückgabewert an, ob ein Überlauf auftrat.`],
    }[mode];
    const operands = op === "neg" ? "the value" : op === "pow" ? "the base and exponent" : "the two operands";
    const operandsDe = op === "neg" ? "den Wert" : op === "pow" ? "Basis und Exponent" : "die beiden Operanden";
    const operationNote = op === "shl" || op === "shr"
      ? mode === "checked"
        ? [`Returns nil when the shift count is at least ${bits}.`, `Gibt nil zurück, wenn die Verschiebung mindestens ${bits} Stellen beträgt.`]
        : mode === "overflowing"
          ? [`The boolean reports whether the shift count is at least ${bits}.`, `Der boolesche Rückgabewert zeigt an, ob die Verschiebung mindestens ${bits} Stellen beträgt.`]
          : ["A shift count greater than the bit width is reduced according to this operation's mode.", "Eine Verschiebung über die Bitbreite hinaus wird gemäß dem Modus dieser Operation behandelt."]
      : op === "neg" && mode === "wrapping" && !operation.startsWith("wrapping_")
        ? ["The ordinary negation can overflow for the smallest signed value.", "Die normale Negation kann beim kleinsten vorzeichenbehafteten Wert überlaufen."]
        : (op === "div" || op === "rem")
          ? [`${modeText[0]} A divisor of zero is invalid.`, `${modeText[1]} Ein Divisor von null ist ungültig.`]
          : modeText;
    symbol.description = `${verb[0]} ${operands} as ${integerType}. ${operationNote[0]}`;
    symbol.description_de = `${verb[1]} ${operandsDe} als ${integerType}. ${operationNote[1]}`;
  } else {
    symbol.description = `Integer operation for ${integerType}, limited to the range ${range}.`;
    symbol.description_de = `Ganzzahloperation für ${integerType} im Wertebereich ${range}.`;
  }

  const paramHelp = {
    value: ["Value to convert or inspect.", "Wert, der umgewandelt oder geprüft wird."],
    radix: ["The current implementation parses decimal text and ignores this declared argument.", "Die aktuelle Implementierung liest Dezimaltext und ignoriert dieses deklarierte Argument."],
    lhs: ["Left operand. For a unary operation, this is the input value.", "Linker Operand. Bei einer einstelligen Operation ist dies der Eingabewert."],
    rhs: operation.endsWith("shl") || operation.endsWith("shr")
      ? ["Number of bit positions to shift.", "Anzahl der Bitpositionen für die Verschiebung."]
      : ["Right operand of the operation.", "Rechter Operand der Operation."],
    count: ["Number of bit positions to rotate.", "Anzahl der Bitpositionen für die Rotation."],
    exp: ["Exponent used by the power operation.", "Exponent für die Potenzoperation."],
  };
  symbol.params = symbol.params.map(param => {
    const paramName = param.name.replace(/\?$/, "");
    const help = paramName === "rhs" && (operation.endsWith("shl") || operation.endsWith("shr"))
      ? ["Number of bit positions to shift.", "Anzahl der Bitpositionen für die Verschiebung."]
      : paramHelp[paramName];
    return {
      ...param,
      description: param.description || help?.[0] || "",
      description_de: help?.[1] || "",
    };
  });

  const args = symbol.params.filter(param => !(operation === "parse" && param.name.replace(/\?$/, "") === "radix")).map(param => {
    if (param.name === "value") return operation === "parse" ? '"42"' : operation === "to_string" ? "42" : "12";
    if (param.name === "radix") return "10";
    if (param.name === "count" || param.name === "rhs") return operation.endsWith("div") || operation.endsWith("rem") ? "3" : "2";
    if (param.name === "exp") return "2";
    return "12";
  });
  if (symbol.kind === "function") {
    const bindings = operation.startsWith("overflowing_") ? "local result, overflowed" : "local result";
    symbol.example = `${bindings} = ${symbol.name}(${args.join(", ")})`;
    symbol.example_de = symbol.example;
  }
}

function addMissingParameterHelp(symbol) {
  const help = (english, german) => [english, german];
  const name = symbol.name;
  const method = name.split(".").at(-1);
  const parameterHelp = {
    str: help("Text whose bytes will be placed in the new buffer.", "Text, dessen Bytes in den neuen Puffer geschrieben werden."),
    value: name === "game.types.of"
      ? help("Lua value whose reflected game type should be returned.", "Lua-Wert, dessen reflektierter Spieltyp zurückgegeben werden soll.")
      : help("Value this function converts, checks, reads, or writes.", "Wert, den diese Funktion umwandelt, prüft, liest oder schreibt."),
    "...": help("Values to include in this log message.", "Werte, die in dieser Mod-Meldung erscheinen sollen."),
    entity: help("Opaque entity handle returned by runtime.ecs.query.", "Opaker Entity-Handle aus runtime.ecs.query."),
    selector: name.startsWith("game.types.")
      ? help("Type name, Type value, or supported type identifier to look up.", "Typname, Type-Wert oder unterstützter Typbezeichner für die Suche.")
      : help("Attribute name or stored ID accepted by this ECS operation.", "Attributname oder gespeicherte ID, die diese ECS-Funktion akzeptiert."),
    options: name === "game.types.find"
      ? help("Optional filters for the type search, such as a name prefix or primitive type.", "Optionale Filter für die Typsuche, zum Beispiel Namensanfang oder primitiver Typ.")
      : help("Optional settings for this request, such as channel, reliability, or result limit.", "Optionale Einstellungen für diese Anfrage, zum Beispiel Kanal, Zustellung oder Treffergrenze."),
    type: help("Reflected game type selected by its name or Type value.", "Reflektierter Spieltyp, ausgewählt über seinen Namen oder Type-Wert."),
    tracking_id: help("Tracking ID passed to the native world operation.", "Tracking-ID für den nativen Weltvorgang."),
    position: help("Three world-space coordinates for this operation.", "Drei Koordinaten in der Spielwelt für diesen Vorgang."),
    guid: help("GUID to convert, inspect, or look up.", "GUID, die umgewandelt, geprüft oder gesucht wird."),
    bytes: help("Binary bytes to decode using the selected reflected type.", "Binärdaten, die mit dem ausgewählten reflektierten Typ gelesen werden."),
    source: help("Existing path inside this mod's export storage.", "Vorhandener Pfad im Exportordner dieses Mods."),
    destination: help("New path inside this mod's export storage.", "Neuer Pfad im Exportordner dieses Mods."),
    qualified_name: help("Exact fully qualified reflected type name.", "Vollständiger qualifizierter Name des reflektierten Typs."),
    feature: help("Feature or operation name to check in the current process.", "Feature- oder Operationsname, der im aktuellen Prozess geprüft wird."),
    request_id: help("Request ID returned by runtime.world.building.submit.", "Anfrage-ID aus runtime.world.building.submit."),
    x: help("Starting voxel coordinate on the X axis.", "Startkoordinate des Voxelbereichs auf der X-Achse."),
    y: help("Starting voxel coordinate on the Y axis.", "Startkoordinate des Voxelbereichs auf der Y-Achse."),
    z: help("Starting voxel coordinate on the Z axis.", "Startkoordinate des Voxelbereichs auf der Z-Achse."),
    size_x: help("Region width in voxels along the X axis.", "Breite des Bereichs in Voxeln entlang der X-Achse."),
    size_y: help("Region height in voxels along the Y axis.", "Höhe des Bereichs in Voxeln entlang der Y-Achse."),
    size_z: help("Region depth in voxels along the Z axis.", "Tiefe des Bereichs in Voxeln entlang der Z-Achse."),
    mod_id: help("Stable ID of the mod to check in the package registry.", "Stabile Mod-ID, die im Paketregister gesucht wird."),
    index: help("Zero-based index in the current reflected type registry.", "Nullbasierter Index im aktuellen Register reflektierter Typen."),
    hash: help("Numeric hash value to compare against the selected hash domain.", "Numerischer Hashwert für die Suche im gewählten Hashbereich."),
    domain: help("Hash namespace to use for the lookup.", "Hashbereich, der für die Suche verwendet wird."),
    parent: help("Type selector to check as the requested parent type.", "Typauswahl, die als übergeordneter Typ geprüft wird."),
    offset: help("Zero-based starting position for this page of results.", "Nullbasierter Startpunkt dieser Ergebnisseite."),
    limit: help("Maximum number of results to return in this call.", "Maximale Anzahl der Ergebnisse dieses Aufrufs."),
    rva: help("Relative virtual address or native binding identifier to inspect.", "Relative virtuelle Adresse oder Kennung der nativen Bindung."),
    id: help("Stable identifier of the feature or intervention to find.", "Stabile Kennung des Features oder Eingriffs, der gesucht wird."),
    keen_entity_id: help("Engine entity ID to resolve into a current runtime handle.", "Engine-Entity-ID, die in einen aktuellen Runtime-Handle aufgelöst wird."),
    component: help("Component name or Type from the current reflected registry.", "Komponentenname oder Type aus dem aktuellen Register."),
    input: help("Building action and its player, item, and world-space data.", "Bauaktion mit Spieler, Gegenstand und Weltkoordinaten."),
    cells: help("Packed material and density value for each voxel in the region.", "Gepackter Material- und Dichtewert für jedes Voxel des Bereichs."),
    operation: help("Name of the runtime operation to check.", "Name der Runtime-Operation, die geprüft werden soll."),
    template_uuid_high_hex: help("High 64-bit half of the entity template UUID in hexadecimal.", "Hohe 64-Bit-Hälfte der Entity-Vorlagen-UUID als Hexadezimaltext."),
    template_uuid_low_hex: help("Low 64-bit half of the entity template UUID in hexadecimal.", "Niedrige 64-Bit-Hälfte der Entity-Vorlagen-UUID als Hexadezimaltext."),
    flags: help("Engine flags passed to the native entity creation operation.", "Engine-Flags für den nativen Vorgang zum Erstellen der Entity."),
    feedback_id: help("Feedback identifier passed to the native placement call.", "Feedback-Kennung für den nativen Platzierungsaufruf."),
    complete: help("Whether the finish-building operation should mark the build as complete.", "Gibt an, ob der Bauvorgang als abgeschlossen markiert werden soll."),
    enabled: help("True to enable the patch, false to restore its original bytes.", "True aktiviert den Patch, false stellt die ursprünglichen Bytes wieder her."),
    notice: help("Notice data containing its ID, title, message, and optional display settings.", "Mitteilungsdaten mit ID, Titel, Text und optionalen Anzeigeeinstellungen."),
    key: help("Setting key declared by this mod.", "Einstellungsschlüssel aus der Mod-Konfiguration."),
    length: help("Number of bytes to reserve, skip, or process.", "Anzahl der Bytes, die reserviert, übersprungen oder verarbeitet werden."),
    src: help("Buffer whose contents should be copied.", "Puffer, dessen Inhalt kopiert werden soll."),
    content_hash: help("Content hash to convert into a GUID.", "Content-Hash, der in eine GUID umgewandelt wird."),
    directory: help("Export directory whose files should be listed.", "Exportordner, dessen Dateien aufgelistet werden."),
    impact_name: help("Exact Impact name of the reflected game type.", "Exakter Impact-Name des reflektierten Spieltyps."),
  };

  symbol.params = (symbol.params || []).map(param => {
    if (param.description?.trim()) return param;
    const paramName = param.name.replace(/\?$/, "");
    let inferred = parameterHelp[paramName];
    if (paramName === "radix") inferred = help("The current implementation parses decimal text and ignores this declared argument.", "Die aktuelle Implementierung liest Dezimaltext und ignoriert dieses deklarierte Argument.");
    if (paramName === "path") inferred = name.includes(".export")
      ? help("Path relative to this mod's export storage.", "Pfad relativ zum Exportordner dieses Mods.")
      : help("Path relative to this mod's package folder.", "Pfad relativ zum Paketordner dieses Mods.");
    if (paramName === "source" && name.includes("export")) inferred = parameterHelp.source;
    if (paramName === "destination" && name.includes("export")) inferred = parameterHelp.destination;
    if (paramName === "position" && name.startsWith("Buffer:")) inferred = help("Zero-based byte position in the buffer.", "Nullbasierte Byteposition im Puffer.");
    if (paramName === "value" && name.startsWith("runtime.ecs.")) inferred = help("Numeric or structured value passed to this ECS operation.", "Numerischer oder strukturierter Wert für diesen ECS-Vorgang.");
    if (paramName === "selector" && name === "runtime.ecs.get_component") inferred = help("Qualified component name or Type from the current registry.", "Qualifizierter Komponentenname oder Type aus dem aktuellen Register.");
    if (paramName === "selector" && name.includes("attribute")) inferred = help("Original attribute name or stored engine attribute ID.", "Ursprünglicher Attributname oder gespeicherte Engine-Attribut-ID.");
    if (paramName === "options" && name === "runtime.network.send") inferred = help("Channel and reliable-delivery settings for this peer message.", "Kanal und Einstellung für zuverlässige Zustellung dieser Peer-Nachricht.");
    if (paramName === "options" && name === "runtime.network.receive") inferred = help("Channel and maximum number of queued messages to read.", "Kanal und maximale Anzahl der gelesenen wartenden Nachrichten.");
    if (paramName === "selector" && name === "runtime.ecs.update_attribute") inferred = help("Original attribute name or stored ID of the root calculation to update.", "Ursprünglicher Attributname oder gespeicherte ID der zu ändernden Root-Berechnung.");
    if (paramName === "source" && name === "Buffer:copy") inferred = help("Buffer whose bytes should be copied.", "Puffer, dessen Bytes kopiert werden sollen.");
    if (paramName === "tracking_id" && name.includes("world.entity")) inferred = help("Engine tracking ID used by this native world operation.", "Engine-Tracking-ID für diesen nativen Weltvorgang.");
    if (!inferred) inferred = help(`Value supplied for the ${paramName} parameter.`, `Wert für den Parameter ${paramName}.`);
    return { ...param, description: inferred[0], description_de: inferred[1] };
  });
}

function publicName(name) {
  const aliases = [
    ["TypeRegistry.", "game.types."],
    ["AssetManager.", "game.assets."],
    ["GuidHelper.", "game.guid."],
    ["log.", "shroudforge.log."],
    ["shroudforge_notifications.", "shroudforge.notifications."],
    ["shroudforge_input.", "shroudforge.input."],
    ["shroudforge_settings.", "shroudforge.settings."],
    ["shroudforge_ui.", "shroudforge.ui."],
  ];
  const alias = aliases.find(([internal]) => name.startsWith(internal));
  return alias ? alias[1] + name.slice(alias[0].length) : name;
}

function publicClassName(name) {
  if (name.startsWith("integer.")) return name;
  return classAliases[name] || name;
}

function namespaceOf(name) {
  if (name.startsWith("game.") || /^(Type|ResolvedStructField|StructField|EnumField|Attribute|Resource|Content)[:.]/.test(name)) return "game";
  if (name.startsWith("runtime.") || /^Runtime[A-Z][^.:]*[.:]/.test(name)) return "runtime";
  if (name.startsWith("shroudforge.")) return "shroudforge";
  return "eml";
}

function walk(directory) {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    const file = path.join(directory, entry.name);
    return entry.isDirectory() ? walk(file) : [file];
  });
}
