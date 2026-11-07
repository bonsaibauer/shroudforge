export function createManifestValidation({ locale }) {
  function validateManifests(manifest, extended) {
    const errors = [];
    const fail = message => errors.push(message);
    if (!manifest || typeof manifest !== "object" || Array.isArray(manifest)) return ["mod.json must start with an object: { ... }."];
    if (!extended || typeof extended !== "object" || Array.isArray(extended)) return ["extended.mod.json must start with an object: { ... }."];
    const checkKeys = (object, allowed, label) => { for (const key of Object.keys(object || {})) if (!allowed.includes(key)) fail(`${label}: ${locale === "de" ? "unbekanntes Feld" : "unknown field"} “${key}”.`); };
    checkKeys(manifest, ["$schema", "id", "name", "version", "authors", "description", "license", "icon", "dependencies", "capabilities"], "mod.json");
    if (manifest["$schema"] !== undefined && typeof manifest["$schema"] !== "string") fail("mod.json: $schema must be a text address.");
    if (typeof manifest.id !== "string" || !/^[A-Za-z0-9._-]{1,80}$/.test(manifest.id)) fail(locale === "de" ? "mod.json: id braucht 1–80 Buchstaben, Zahlen, Punkte, Bindestriche oder Unterstriche." : "mod.json: id must use 1–80 letters, numbers, dots, hyphens, or underscores.");
    if (typeof manifest.name !== "string" || manifest.name.length < 1 || manifest.name.length > 120) fail("mod.json: name must contain 1–120 characters.");
    if (typeof manifest.version !== "string" || !/^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(manifest.version)) fail("mod.json: version must look like 1.0.0.");
    if (manifest.description !== undefined && (typeof manifest.description !== "string" || manifest.description.length > 2000)) fail("mod.json: description allows up to 2,000 characters.");
    if (manifest.authors !== undefined && (!Array.isArray(manifest.authors) || manifest.authors.some(value => typeof value !== "string" || !value) || new Set(manifest.authors).size !== manifest.authors.length)) fail("mod.json: authors must be a list of unique, non-empty names.");
    if (manifest.license !== undefined && manifest.license !== null && typeof manifest.license !== "string") fail("mod.json: license must be text or null.");
    if (manifest.icon !== undefined && manifest.icon !== null && (typeof manifest.icon !== "string" || !/^(?:assets\/[A-Za-z0-9._/-]+|[A-Za-z0-9_-][A-Za-z0-9._-]*)$/.test(manifest.icon))) fail("mod.json: icon must be a file name or a path under assets/.");
    if (manifest.capabilities !== undefined && (!Array.isArray(manifest.capabilities) || manifest.capabilities.some(value => !["patch", "export", "runtime", "runtime-register-dll"].includes(value)) || new Set(manifest.capabilities).size !== manifest.capabilities.length)) fail("mod.json: capabilities can contain patch, export, runtime, and runtime-register-dll once each.");
    if (manifest.dependencies !== undefined) {
      if (!Array.isArray(manifest.dependencies)) fail("mod.json: dependencies must be a list.");
      else {
        const dependencyIds = new Set();
        for (const item of manifest.dependencies) {
          if (!item || typeof item !== "object" || Array.isArray(item)) { fail("mod.json: each dependency must be an object with an id and version."); continue; }
          if (Object.keys(item).some(key => !["id", "version", "optional"].includes(key))) fail("mod.json: a dependency only allows id, version, and optional.");
          if (typeof item.id !== "string" || !/^[A-Za-z0-9._-]{1,80}$/.test(item.id)) fail("mod.json: dependency id must use 1–80 letters, numbers, dots, hyphens, or underscores.");
          else {
            if (item.id === manifest.id) fail("mod.json: a mod cannot depend on itself.");
            if (dependencyIds.has(item.id)) fail(`mod.json: dependency “${item.id}” is listed more than once.`);
            dependencyIds.add(item.id);
          }
          if (typeof item.version !== "string" || !item.version.trim()) fail("mod.json: every dependency needs a non-empty SemVer version requirement.");
          if (item.optional !== undefined && typeof item.optional !== "boolean") fail("mod.json: dependency optional must be true or false.");
        }
      }
    }
    checkKeys(extended, ["$schema", "schemaVersion", "enabled", "targets", "launcher", "links", "changelog", "settings", "groups", "presets"], "extended.mod.json");
    if (extended.schemaVersion !== 1) fail("extended.mod.json: schemaVersion must be 1.");
    if (extended["$schema"] !== undefined && typeof extended["$schema"] !== "string") fail("extended.mod.json: $schema must be a text address.");
    if (typeof extended.enabled !== "boolean") fail("extended.mod.json: enabled must be true or false.");
    if (extended.targets !== undefined && (!Array.isArray(extended.targets) || extended.targets.length < 1 || extended.targets.some(target => !["client", "server"].includes(target)) || new Set(extended.targets).size !== extended.targets.length)) fail("extended.mod.json: targets must contain client, server, or both once each.");
    if (extended.launcher !== undefined && !["EML", "SF"].includes(extended.launcher)) fail("extended.mod.json: launcher must be EML or SF.");
    if (extended.links !== undefined && (!extended.links || typeof extended.links !== "object" || Array.isArray(extended.links) || Object.entries(extended.links).some(([key, url]) => !/^[a-z][a-z0-9-]*$/.test(key) || typeof url !== "string" || !url.startsWith("https://") || url.length > 2048))) fail("extended.mod.json: link names must be lowercase and each link must be an HTTPS address.");
    if (extended.changelog !== undefined && (!Array.isArray(extended.changelog) || extended.changelog.length > 50 || extended.changelog.some(item => typeof item !== "string" || !item || item.length > 500))) fail("extended.mod.json: changelog allows up to 50 notes, each 1–500 characters.");
    const settings = extended.settings || {};
    if (extended.settings !== undefined && (!settings || typeof settings !== "object" || Array.isArray(settings))) fail("extended.mod.json: settings must be an object.");
    const controls = ["toggle", "checkbox", "text", "textarea", "number", "slider", "select", "radio", "segmented", "multiselect", "keybind", "color"];
    for (const [key, setting] of Object.entries(settings)) {
      if (!/^[A-Za-z0-9._-]{1,80}$/.test(key)) fail(`Setting “${key}”: use 1–80 letters, numbers, dots, hyphens, or underscores.`);
      if (setting && typeof setting === "object" && !Array.isArray(setting)) {
        checkKeys(setting, ["value", "label", "description", "control", "min", "max", "step", "minLength", "maxLength", "options"], `Setting “${key}”`);
        if (!("value" in setting) || !["boolean", "string", "number"].includes(typeof setting.value) && !Array.isArray(setting.value)) fail(`Setting “${key}” needs a value.`);
        if (setting.label !== undefined && (typeof setting.label !== "string" || !setting.label || setting.label.length > 120)) fail(`Setting “${key}”: label allows 1–120 characters.`);
        if (setting.description !== undefined && (typeof setting.description !== "string" || setting.description.length > 500)) fail(`Setting “${key}”: description allows up to 500 characters.`);
        if (setting.control !== undefined && !controls.includes(setting.control)) fail(`Setting “${key}”: choose one of the listed controls.`);
        if (setting.control === "keybind" && (typeof setting.value !== "string" || setting.value.length > 96 || !/^(?:(?:Ctrl\+)?(?:Alt\+)?(?:Shift\+)?(?:Meta\+)?SDL:[1-9][0-9]{0,2}(?:\|(?:Ctrl\+)?(?:Alt\+)?(?:Shift\+)?(?:Meta\+)?Mouse:[1-9][0-9]?)?|(?:Ctrl\+)?(?:Alt\+)?(?:Shift\+)?(?:Meta\+)?Mouse:[1-9][0-9]?)?$/.test(setting.value))) fail(locale === "de" ? `Einstellung „${key}“: Ein Tastenkürzel braucht eine Zeichenfolge wie SDL:64, Mouse:4 oder Ctrl+SDL:29|Mouse:4. Maximal eine Tastatur- und eine Mausbelegung, Tastatur zuerst. Leer deaktiviert es. Zusatztasten stehen einmalig in der Reihenfolge Ctrl, Alt, Shift, Meta.` : `Setting “${key}”: a keybind requires a string such as SDL:64, Mouse:4 or Ctrl+SDL:29|Mouse:4. At most one keyboard and one mouse chord, keyboard first. Empty disables it. Modifier prefixes appear once in Ctrl, Alt, Shift, Meta order.`);
        for (const field of ["min", "max"]) if (setting[field] !== undefined && typeof setting[field] !== "number") fail(`Setting “${key}”: ${field} must be a number.`);
        if (setting.step !== undefined && (typeof setting.step !== "number" || setting.step <= 0)) fail(`Setting “${key}”: step must be greater than zero.`);
        for (const field of ["minLength", "maxLength"]) if (setting[field] !== undefined && (!Number.isInteger(setting[field]) || setting[field] < 0)) fail(`Setting “${key}”: ${field} must be zero or a positive whole number.`);
        if (setting.options !== undefined && (!setting.options || typeof setting.options !== "object" || Array.isArray(setting.options) || !Object.keys(setting.options).length || Object.values(setting.options).some(value => typeof value !== "string" || !value || value.length > 120))) fail(`Setting “${key}”: options need at least one named choice.`);
      } else if (!["boolean", "string", "number"].includes(typeof setting) && !Array.isArray(setting)) fail(`Setting “${key}” needs a text, number, switch, list, or setting object.`);
    }
    if (extended.groups !== undefined && !Array.isArray(extended.groups)) fail("extended.mod.json: groups must be a list.");
    const groups = Array.isArray(extended.groups) ? extended.groups : [];
    for (const group of groups) {
      if (!group || typeof group !== "object" || Array.isArray(group)) { fail("Each group must be an object."); continue; }
      checkKeys(group, ["label", "description", "settings", "actions"], "Group");
      if (typeof group.label !== "string" || !group.label || group.label.length > 120) fail("Group: label allows 1–120 characters.");
      if (group.description !== undefined && (typeof group.description !== "string" || group.description.length > 500)) fail("Group: description allows up to 500 characters.");
      if (group.settings !== undefined && (!Array.isArray(group.settings) || group.settings.some(key => typeof key !== "string" || !/^[A-Za-z0-9._-]{1,80}$/.test(key) || !(key in settings)) || new Set(group.settings).size !== group.settings.length)) fail("Group: settings must be unique names that exist in the settings list.");
      if (group.actions !== undefined && (!Array.isArray(group.actions) || group.actions.some(action => !action || typeof action !== "object" || Array.isArray(action) || Object.keys(action).some(key => !["id", "label", "style", "confirm"].includes(key)) || typeof action.id !== "string" || !/^[A-Za-z0-9._-]{1,80}$/.test(action.id) || typeof action.label !== "string" || !action.label || action.label.length > 120 || (action.style !== undefined && !["primary", "secondary", "danger"].includes(action.style)) || (action.confirm !== undefined && (typeof action.confirm !== "string" || action.confirm.length > 500))))) fail("Group: each action needs an id and label; style and confirmation are optional.");
    }
    if (extended.presets !== undefined && (!extended.presets || typeof extended.presets !== "object" || Array.isArray(extended.presets))) fail("extended.mod.json: presets must be an object.");
    for (const [key, preset] of Object.entries(extended.presets && typeof extended.presets === "object" ? extended.presets : {})) {
      if (!(key in settings)) fail(`Preset “${key}”: the key must name a setting.`);
      if (!preset || typeof preset !== "object" || Array.isArray(preset)) { fail(`Preset “${key}” must be an object.`); continue; }
      checkKeys(preset, ["custom", "settings", "values"], `Preset “${key}”`);
      if (preset.custom !== undefined && (typeof preset.custom !== "string" || !preset.custom)) fail(`Preset “${key}”: custom must be a non-empty text.`);
      const members = Array.isArray(preset.settings) ? preset.settings : [];
      if (!members.length || members.some(member => typeof member !== "string" || !(member in settings) || member === key) || new Set(members).size !== members.length) fail(`Preset “${key}”: settings must be unique names of other settings.`);
      const values = preset.values && typeof preset.values === "object" && !Array.isArray(preset.values) ? preset.values : null;
      if (!values || !Object.keys(values).length) { fail(`Preset “${key}”: values need at least one preset.`); continue; }
      for (const [name, entry] of Object.entries(values)) if (!entry || typeof entry !== "object" || Array.isArray(entry) || members.some(member => !(member in entry)) || Object.keys(entry).some(member => !members.includes(member))) fail(`Preset “${key}”: “${name}” must give a value to each of its settings and to no other.`);
    }
    return errors;
  }

  return { validateManifests };
}
