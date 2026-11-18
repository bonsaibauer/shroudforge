import fs from "node:fs";
import path from "node:path";

export function readApiVersion(root) {
  const workspaceManifest = fs.readFileSync(path.join(root, "Cargo.toml"), "utf8");
  const runtimeManifest = fs.readFileSync(path.join(root, "modules", "mod-runtime", "Cargo.toml"), "utf8");
  if (!/^name\s*=\s*"shroudforge-api"\s*$/m.test(runtimeManifest) || !/^version\.workspace\s*=\s*true\s*$/m.test(runtimeManifest)) {
    throw new Error("The shroudforge-api package version source changed. Update the website API version reader.");
  }

  const packageSection = workspaceManifest.match(/^\[workspace\.package\]\s*\r?\n((?:(?!^\[).*(?:\r?\n|$))*)/m)?.[1];
  const version = packageSection?.match(/^version\s*=\s*"(\d+\.\d+\.\d+)"\s*$/m)?.[1];
  if (!version) throw new Error("Cargo.toml must declare a semantic version in [workspace.package].");
  return version;
}
