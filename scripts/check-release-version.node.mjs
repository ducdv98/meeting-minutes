import assert from "node:assert/strict";
import test from "node:test";

import { checkReleaseVersion, readVersions } from "./check-release-version.mjs";

test("the current release tag matches all version files", () => {
  const versions = readVersions();
  checkReleaseVersion(`v${versions["src-tauri/tauri.conf.json"]}`, versions);
});

test("a mismatched version fails with its source", () => {
  assert.throws(
    () => checkReleaseVersion("v0.1.0", {
      "src-tauri/tauri.conf.json": "0.1.0",
      "package.json": "0.1.1",
      "src-tauri/Cargo.toml": "0.1.0",
    }),
    /package.json version 0.1.1 does not match tag v0.1.0/,
  );
});

test("a malformed tag fails", () => {
  assert.throws(
    () => checkReleaseVersion("release-0.1.0", readVersions()),
    /Expected a vX.Y.Z tag/,
  );
});

test("a test tag is accepted when its version matches", () => {
  checkReleaseVersion("v0.1.0-test", {
    "src-tauri/tauri.conf.json": "0.1.0-test",
    "package.json": "0.1.0-test",
    "src-tauri/Cargo.toml": "0.1.0-test",
  });
});
