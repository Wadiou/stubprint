# receipt-printer-emulator

A fake receipt printer for development. It listens on TCP like a network printer, reads the
commands it receives, and shows what would have been printed, in a web page and through a REST API
so people and agents can both use it.

Status: the ESC/POS decoder in `crates/core` is done and tested against generated fixtures. The
server and the web page are next. See [docs/PLAN.md](docs/PLAN.md) for the plan and
[docs/ESCPOS.md](docs/ESCPOS.md) for what is decoded.

```bash
# generate the fixtures once, then run the tests
cd fixtures/generate && npm install && npm run generate && cd ../..
cargo test --workspace
```

## Licence

MIT, see [LICENSE](LICENSE). The captured streams in `fixtures/captured` keep the licences of the projects they come from, see [NOTICE](fixtures/captured/NOTICE.md).
