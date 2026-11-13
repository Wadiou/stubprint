# Landing page

The page of [stubprint.dev](https://stubprint.dev): React and Vite, with plain CSS. It is not part of
the Rust workspace or of the web page of the program in `web/`.

```bash
pnpm install
pnpm dev        # the page, with reload
pnpm build      # typecheck, then dist/
pnpm brand      # redraw the icons and og.png in public/
pnpm receipt    # redraw the receipt of the page, src/assets/receipt.png
```

- The colours, the fonts and the square corners are in `../ui/theme.css`, which the web page of the
  program imports too, so the two look the same. Change them there.
- The name, the headline and the colours of the cards are in `src/brand.ts`. The page and `pnpm brand` both
  read it, so the page and the card that links show cannot differ.
- The receipt is a picture that Stubprint drew, not a copy typed by hand: the `cafe` sample as the
  default printer prints it, which is what the command on the page does. `pnpm receipt` draws it
  again when the sample or the program's drawing changes. It needs ImageMagick, and a built
  `stubprint` (`STUBPRINT=path`, else the debug build of the workspace).
- The version on the page is the one in the `Cargo.toml` of the workspace.
- `pnpm brand` draws in a headless Chromium, with the typefaces of the page. Playwright is pinned
  to the version whose browser is installed here, so run `pnpm exec playwright install chromium`
  once if it cannot find one.
- `public/` holds the generated files (`favicon.ico`, `favicon-32x32.png`, `apple-touch-icon.png`,
  `og.png`) next to `robots.txt` and `sitemap.xml`. Commit them after `pnpm brand`.
- The install commands on the page point at `stubprint.dev/install.sh`, `install.ps1` and
  `<version>/install.sh`. `middleware.ts` answers those addresses on Vercel (Root Directory
  `landing`, no other config): it checks the version, asks GitHub whether the release exists, and
  redirects to the installer of the release, or says in words that there is none. It is one file, since
  Vercel compiles it alone.
