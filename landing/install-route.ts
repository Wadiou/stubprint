import { REPO } from "./src/brand.ts";

/** The installers that a release attaches, by the name the address asks for. */
const INSTALLERS = { "install.sh": "stubprint-installer.sh", "install.ps1": "stubprint-installer.ps1" } as const;

/** A version as the releases are called: three numbers, with no `v`. */
const VERSION = /^\d+\.\d+\.\d+$/;

export type Route = { redirect: string } | { status: number; message: string };

/**
 * Where the address of an installer leads: `/install.sh` and `/latest/install.sh` to the installer
 * of the latest release, and `/0.4.0/install.sh` to the one of that version. Anything else in the
 * place of the version is told what is wanted. An address that is not an installer is not routed.
 */
export function route(pathname: string): Route | undefined {
  const parts = pathname.split("/").filter(Boolean);
  const name = INSTALLERS[parts.at(-1) as keyof typeof INSTALLERS];
  if (!name || parts.length > 2) return undefined;

  const version = parts.length === 2 ? parts[0] : "latest";
  if (version === "latest") return { redirect: `${REPO}/releases/latest/download/${name}` };
  if (!version || !VERSION.test(version)) {
    return { status: 404, message: `"${version}" is not a version like 0.4.0, or latest.\n` };
  }
  return { redirect: `${REPO}/releases/download/v${version}/${name}` };
}

/**
 * The answer to a request for an installer. A release that does not exist is told, in words, and
 * not as the bare 404 of GitHub. When GitHub cannot be asked, the redirect is given anyway.
 */
export async function answer(request: Request): Promise<Response | undefined> {
  const { pathname } = new URL(request.url);
  const found = route(pathname);
  if (!found) return undefined;
  if ("message" in found) return text(found.message, found.status);

  try {
    const asked = await fetch(found.redirect, { method: "HEAD" });
    if (asked.status === 404) {
      const version = pathname.split("/").filter(Boolean)[0];
      const which = VERSION.test(version ?? "") ? `Stubprint ${version}` : "a release of Stubprint";
      return text(`There is no installer for ${which}: see ${REPO}/releases\n`, 404);
    }
  } catch {
    // GitHub did not answer, and the installer may still be there.
  }
  return Response.redirect(found.redirect, 302);
}

function text(message: string, status: number) {
  return new Response(message, { status, headers: { "content-type": "text/plain; charset=utf-8" } });
}
