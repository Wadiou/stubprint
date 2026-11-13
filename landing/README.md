# Landing page

The page of [stubprint.dev](https://stubprint.dev): React and Vite, with plain CSS. It is not part of
the Rust workspace or of the web page of the program in `web/`.

```bash
pnpm install
pnpm dev        # the page, with reload
pnpm build      # typecheck, then dist/
pnpm brand      # redraw the icons and og.png in public/
```

- The name, the headline, the colours and the receipt it shows are in `src/brand.ts`. The page
  and `pnpm brand` both read it, so the page and the card that links show cannot differ.
- The version on the page is the one in the `Cargo.toml` of the workspace.
- `pnpm brand` draws in a headless Chromium, with the typefaces of the page. Playwright is pinned
  to the version whose browser is installed here, so run `pnpm exec playwright install chromium`
  once if it cannot find one.
- `public/` holds the generated files (`favicon.ico`, `favicon-32x32.png`, `apple-touch-icon.png`,
  `og.png`) next to `robots.txt` and `sitemap.xml`. Commit them after `pnpm brand`.
- The install commands on the page point at `stubprint.dev/install.sh`, `install.ps1` and
  `<version>/install.sh`. They work once the release publishes the installers and the site
  redirects those paths to them.
