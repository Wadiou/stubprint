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
