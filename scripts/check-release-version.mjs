import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));

export function checkReleaseVersion(tag, versions) {
  if (!/^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(tag)) {
    throw new Error(`Expected a vX.Y.Z tag (optionally with a prerelease suffix), got ${tag}`);
  }
  const expected = tag.slice(1);
  for (const [source, version] of Object.entries(versions)) {
    if (version !== expected) {
      throw new Error(`${source} version ${version} does not match tag ${tag}`);
    }
  }
}

export function readVersions() {
  const config = JSON.parse(readFileSync(`${root}src-tauri/tauri.conf.json`, "utf8"));
  const pkg = JSON.parse(readFileSync(`${root}package.json`, "utf8"));
  const cargo = readFileSync(`${root}src-tauri/Cargo.toml`, "utf8");
  const packageSection = cargo.split(/^\[package\]\s*$/m)[1]?.split(/^\[/m)[0];
  const cargoVersion = packageSection?.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  if (!cargoVersion) {
    throw new Error("Could not read package version from src-tauri/Cargo.toml");
  }
  return {
    "src-tauri/tauri.conf.json": config.version,
    "package.json": pkg.version,
    "src-tauri/Cargo.toml": cargoVersion,
  };
}

if (process.argv[1] && fileURLToPath(import.meta.url) === resolve(process.argv[1])) {
  try {
    const tag = process.env.RELEASE_TAG ?? process.env.GITHUB_REF_NAME ?? process.argv[2] ?? "";
    checkReleaseVersion(tag, readVersions());
    console.log(`Release tag ${tag} matches all app versions.`);
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
