import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { answer, route } from "./install-route.ts";

const REPO = "https://github.com/Wadiou/stubprint";

test("the latest installer is the one of /install.sh and /latest/install.sh", () => {
  for (const path of ["/install.sh", "/latest/install.sh"]) {
    assert.deepEqual(route(path), { redirect: `${REPO}/releases/latest/download/stubprint-installer.sh` });
  }
  assert.deepEqual(route("/install.ps1"), { redirect: `${REPO}/releases/latest/download/stubprint-installer.ps1` });
});

test("a version leads to the installer of that release", () => {
  assert.deepEqual(route("/0.4.0/install.sh"), { redirect: `${REPO}/releases/download/v0.4.0/stubprint-installer.sh` });
  assert.deepEqual(route("/10.20.30/install.ps1"), { redirect: `${REPO}/releases/download/v10.20.30/stubprint-installer.ps1` });
});

test("something that is not a version is told what is wanted", () => {
  for (const bad of ["1.2", "v0.4.0", "x.y.z", "1.2.3-rc.1"]) {
    const found = route(`/${bad}/install.sh`);
    assert.ok(found && "status" in found && found.status === 404, bad);
  }
});

test("an address that is not an installer is not routed", () => {
  for (const path of ["/", "/index.html", "/0.4.0/readme", "/a/b/install.sh", "/../0.4.0/install.sh", "/install.sh.map"]) {
    assert.equal(route(path), undefined, path);
  }
});

const real = globalThis.fetch;
afterEach(() => {
  globalThis.fetch = real;
});

test("a release that exists is redirected to", async () => {
  globalThis.fetch = async () => new Response(null, { status: 200 });
  const response = await answer(new Request("https://stubprint.dev/0.4.0/install.sh"));
  assert.equal(response?.status, 302);
  assert.equal(response?.headers.get("location"), `${REPO}/releases/download/v0.4.0/stubprint-installer.sh`);
});

test("a release that does not exist is said so in words", async () => {
  globalThis.fetch = async () => new Response(null, { status: 404 });
  const response = await answer(new Request("https://stubprint.dev/9.9.9/install.sh"));
  assert.equal(response?.status, 404);
  assert.match((await response?.text()) ?? "", /no installer for Stubprint 9\.9\.9/);
});

test("when GitHub cannot be asked, the redirect is given anyway", async () => {
  globalThis.fetch = async () => {
    throw new Error("offline");
  };
  const response = await answer(new Request("https://stubprint.dev/install.sh"));
  assert.equal(response?.status, 302);
});
