import { answer } from "./install-route.ts";

// Runs on Vercel before the files of the page, for the addresses that people type into curl and
// PowerShell. The page itself is not asked about them.
export const config = {
  matcher: ["/install.sh", "/install.ps1", "/:version/install.sh", "/:version/install.ps1"],
};

export default function middleware(request: Request) {
  return answer(request);
}
