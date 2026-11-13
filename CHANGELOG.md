# [0.4.0](https://github.com/Wadiou/stubprint/compare/v0.3.0...v0.4.0) (2026-11-13)


### Features

* **server:** limit how many programs a printer takes and close the ones that go quiet ([829029c](https://github.com/Wadiou/stubprint/commit/829029c77a3be4ca1f2e00e53304651db40e8990))
* **web:** set the connection limit and the idle time of a printer ([05c46d6](https://github.com/Wadiou/stubprint/commit/05c46d6232f726ed9e5eab93e500a119207bde7d))

# [0.3.0](https://github.com/Wadiou/stubprint/compare/v0.2.0...v0.3.0) (2026-11-13)


### Bug Fixes

* **web:** open the live events again when the browser closed them ([7343e0f](https://github.com/Wadiou/stubprint/commit/7343e0f4b7eaa904b10b764f39ed3141c92a18ab))


### Features

* **core:** answer the maintenance counters of an Epson printer ([7d63954](https://github.com/Wadiou/stubprint/commit/7d639544478d37939b0cac4f79174031ebd0f62a))
* **core:** say which command ends each receipt and which bytes made each command ([8951876](https://github.com/Wadiou/stubprint/commit/895187685e8c5673afc70e9bca10d78edd85f00f))
* **render:** count the paper that came out after each command ([050543c](https://github.com/Wadiou/stubprint/commit/050543cdc6f769680f983c6865d25a1714d41684))
* **render:** tell how tall the paper is without drawing it ([7576318](https://github.com/Wadiou/stubprint/commit/7576318f9feb68774968a23c0612d5faa85fdc38))
* **server:** give a printer a paper roll, counters and a drawer that opens when it is kicked ([f77cba4](https://github.com/Wadiou/stubprint/commit/f77cba46011729545a3761fa68ec3366a365cdff))
* **server:** make a printer take time to print, at a speed set in millimetres a second ([08d3651](https://github.com/Wadiou/stubprint/commit/08d365197087d2897669bd874c85f80798366107))
* **server:** print over time, so a receipt begins, grows and ends as the printer works ([66071b7](https://github.com/Wadiou/stubprint/commit/66071b7c39130382bfa17a9091d6f404b7fc5a1d))
* **web:** follow a receipt as the printer prints it ([aca66ea](https://github.com/Wadiou/stubprint/commit/aca66ea1faccd76d82c2e38ac2bcc285018da636))
* **web:** set the print speed of a printer ([2c31f8c](https://github.com/Wadiou/stubprint/commit/2c31f8c6e247b868b34d3aaa9249fdce0fb99709))
* **web:** show the paper roll and the counters of a printer ([0a48cbc](https://github.com/Wadiou/stubprint/commit/0a48cbc80cb1db7b773fdeca9c7b0511e31d88ae))

# [0.2.0](https://github.com/Wadiou/stubprint/compare/v0.1.0...v0.2.0) (2026-11-12)


### Bug Fixes

* **server:** let a connection that is open follow how much a printer holds ([f4ae8d4](https://github.com/Wadiou/stubprint/commit/f4ae8d40cf59700f63b677b104252a6cbe34ea18))
* **web:** make the API client the one the server describes again ([1b98f31](https://github.com/Wadiou/stubprint/commit/1b98f310fc8788ab72a631d4dff4cb07122d54ce))


### Features

* **core:** hold what a printer receives while it cannot print ([28ee015](https://github.com/Wadiou/stubprint/commit/28ee0152d52a35b5aaf33cb4f571cdb1deade785))
* **server:** hold what a printer receives by default while it cannot print ([37e322b](https://github.com/Wadiou/stubprint/commit/37e322bffb591b9e57af0605b4bc5ae1a7f4c3b6))
* **server:** keep the printers and the address of the API in one config file ([f456a7b](https://github.com/Wadiou/stubprint/commit/f456a7b749cdfdd6874d67dfc1388d332a09c003))
* **server:** keep the printers between runs ([b7dc818](https://github.com/Wadiou/stubprint/commit/b7dc81889adadfebd1ad5202d3d963a1db183168))
* **server:** keep the printers, the receipts and the connections in one SQLite database ([da9b220](https://github.com/Wadiou/stubprint/commit/da9b2200446f9b125c730e0b9846b4c08944efa0))
* **server:** let a printer hold what it is sent while it cannot print ([92b84ab](https://github.com/Wadiou/stubprint/commit/92b84abaca73eeca2dd546886e24a6a16854659e))
* **server:** let a printer limit how much the network keeps for a program ([a094c82](https://github.com/Wadiou/stubprint/commit/a094c827314285ae595a07cd9af8edbafa5b52f4))
* **server:** let what is written for a printer win over what was kept, setting by setting ([697f03d](https://github.com/Wadiou/stubprint/commit/697f03dbb29e790c51bdb9c6b382a80ede75d9a0))
* **server:** list the addresses of the computer ([c1ce7a7](https://github.com/Wadiou/stubprint/commit/c1ce7a73eb40f81a2861699c87f44353ae648988))
* **server:** tell each change of a connection as it happens ([7ee1cfc](https://github.com/Wadiou/stubprint/commit/7ee1cfca6eef5f47fef018fe9ca8132bc88c3a7d))
* **server:** tell how many bytes a printer holds ([57c3f1b](https://github.com/Wadiou/stubprint/commit/57c3f1bb9f1360394a0aac7857628a61e8fb037c))
* **web:** choose how much a printer holds while it cannot print ([d587f3b](https://github.com/Wadiou/stubprint/commit/d587f3be05aba39f7ca9145451bb970201dd95e1))
* **web:** choose how much the network keeps for a program ([385f26b](https://github.com/Wadiou/stubprint/commit/385f26bc2e9c095eaf95142a2cee10d0635b75b6))
* **web:** show a pointer over the rows of a table that can choose one ([2609edd](https://github.com/Wadiou/stubprint/commit/2609edd93d8e2995d0286d6fa1c6a11d613d03db))
* **web:** show a toast when a receipt is printed ([3ea929b](https://github.com/Wadiou/stubprint/commit/3ea929bdd613f5b65a920379d42b6879509cf8b0))
* **web:** show how full the buffer of a printer is ([2eeea19](https://github.com/Wadiou/stubprint/commit/2eeea1986483bfd714cbb2c4a1944096c2493272))
* **web:** show the connections of a printer live and check what comes from the server ([f5aa1eb](https://github.com/Wadiou/stubprint/commit/f5aa1eba7c9c6444f6608055764e35463957aeaa))
* **web:** tell when a print is stuck because the printer cannot print ([4682faa](https://github.com/Wadiou/stubprint/commit/4682faaaca07651e1eb1510999805fb3aca4638b))
* **web:** type where a printer listens and show how to reach it ([a7ea066](https://github.com/Wadiou/stubprint/commit/a7ea0661ff437266d7989b0fb3fa45c7ef8a61c2))

# [0.1.0](https://github.com/Wadiou/stubprint/compare/v0.0.1...v0.1.0) (2026-10-08)


### Bug Fixes

* **repo:** let the crates depend on each other by path alone ([829e6ec](https://github.com/Wadiou/stubprint/commit/829e6ec56bf6a8949fba3c3bbbd6d386649a82cc))
* **server:** describe every route and answer the routes of the page ([5038f39](https://github.com/Wadiou/stubprint/commit/5038f390b1915bc1e434ee04009393d770834c4b))
* **server:** stop a printer that is told to stop before its port is served ([3280830](https://github.com/Wadiou/stubprint/commit/3280830eaf2657679c4ec749dd77b35b87e7344c))
* **server:** stop while a page is watching the live events ([89d43c0](https://github.com/Wadiou/stubprint/commit/89d43c02d7870c36266bb85ac9d44fb92ad0dcf9))


### Features

* **core:** say how wide the paper is in millimetres, its dpi and its manuals ([7766740](https://github.com/Wadiou/stubprint/commit/7766740369bc4416d3d6bb12b0d8a72651d5160c))
* **core:** take the printer profiles from a pinned release of the database ([f71e942](https://github.com/Wadiou/stubprint/commit/f71e94298b2cabbb109515f7bc06d9bf6fbcde75))
* **server:** carry the web page inside the program ([175a4d1](https://github.com/Wadiou/stubprint/commit/175a4d102109d5c96e1e1e6cb321b2bc15b3436b))
* **server:** give a printer that is added a free port and a free name ([8295588](https://github.com/Wadiou/stubprint/commit/82955889550c7c149348e914baa782a816d8ce57))
* **server:** give each form of a receipt its own route ([4de4ef3](https://github.com/Wadiou/stubprint/commit/4de4ef3b2e4297e6abc94e72c5557417ca044d5d))
* **server:** give the bytes, a picture and the text of a sample at routes of their own ([c719a6e](https://github.com/Wadiou/stubprint/commit/c719a6e89df44e4528e274940b59aac2662bb89c))
* **server:** keep the pictures of a receipt that were drawn ([4fb44f0](https://github.com/Wadiou/stubprint/commit/4fb44f0b0b3b5ee73c43c55b7f15afce9aea98f2))
* **server:** remove one receipt ([3e77396](https://github.com/Wadiou/stubprint/commit/3e773960c9e18102c1964ff36685769a7e1d8e3f))
* **server:** say what each printer model is in the list of profiles ([0cd183a](https://github.com/Wadiou/stubprint/commit/0cd183a6c22ea363e2bafe42f527223cf11c8f98))
* **server:** say which printer model a sample is written for ([ea71484](https://github.com/Wadiou/stubprint/commit/ea714846eacef2cec06b42aff02033f2d3a06f12))
* **server:** set a printer to another model by request ([624d233](https://github.com/Wadiou/stubprint/commit/624d23390da76c3b549d7f9c69c377192e328de8))
* **server:** write where the server listens to a ready file ([bdb0099](https://github.com/Wadiou/stubprint/commit/bdb0099c5eea25e9693eb049e11388cd718c516b))
* **web:** add the printer of a sample from its card and tell with a toast that it was printed ([cfa879a](https://github.com/Wadiou/stubprint/commit/cfa879aa4d32175ba6c2da66ea72e52e690d946f))
* **web:** choose the printer in the header and add and set up printers ([3c08d63](https://github.com/Wadiou/stubprint/commit/3c08d63d4415ed5b58f2cdfc7e3a6c9b049d12e2))
* **web:** delete the receipt that is shown, after asking ([5aa9453](https://github.com/Wadiou/stubprint/commit/5aa9453f0a15b31aa235cc50780edd79b8c808d7))
* **web:** fill in the name, pick a free port and choose the model from a list ([fbca9fc](https://github.com/Wadiou/stubprint/commit/fbca9fc8314338ad641686095e19d742eb58deaa))
* **web:** list the receipts on the live paper and name the printer model ([b086c59](https://github.com/Wadiou/stubprint/commit/b086c59b2aa811b541f564bc85f9986b2067251b))
* **web:** make the empty live paper a way to try a sample ([4282a0c](https://github.com/Wadiou/stubprint/commit/4282a0c0dcee139cf518a3a8f84294d5b154793f))
* **web:** name each command in words and list what the printer believed ([e2588be](https://github.com/Wadiou/stubprint/commit/e2588be60610eac048b9109e3a9e8b11bafb9ee8))
* **web:** play a receipt as it was built ([e753727](https://github.com/Wadiou/stubprint/commit/e753727d600dd03ac1ba26b9aae251c49e80dff4))
* **web:** print a sample to the printer it was made for ([c1c4330](https://github.com/Wadiou/stubprint/commit/c1c4330c71adf1f99c8543d061adbcc6e5f66c21))
* **web:** replay a receipt command by command on the live paper ([47758d8](https://github.com/Wadiou/stubprint/commit/47758d8c0de8cc5d4ac992683c284861e19a62ac))
* **web:** scaffold the web page on the API of the server ([b708838](https://github.com/Wadiou/stubprint/commit/b70883806d4592454f260ded211a5294b93cb108))
* **web:** show the live paper as a picture, text or bytes ([9700b2b](https://github.com/Wadiou/stubprint/commit/9700b2b3a1a8d9fb75e3fc401b9417e6ecc999e5))
* **web:** show the samples and print them ([70164fd](https://github.com/Wadiou/stubprint/commit/70164fda84316174a69c7168cbc02671d83fb57b))
* **web:** take the server of the development proxy from STUBPRINT_SERVER ([e5ad805](https://github.com/Wadiou/stubprint/commit/e5ad805b16c6379c3699539793f106d0a684dcd1))
