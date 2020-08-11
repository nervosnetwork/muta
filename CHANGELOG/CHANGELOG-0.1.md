

## [0.1.2-beta](https://github.com/nervosnetwork/muta/compare/v0.1.2-beta2...v0.1.2-beta) (2020-06-04)



## [0.1.2-beta2](https://github.com/nervosnetwork/muta/compare/v0.1.2-beta1...v0.1.2-beta2) (2020-06-03)


### Features

* supported storage metrics ([#307](https://github.com/nervosnetwork/muta/issues/307)) ([2531b8d](https://github.com/nervosnetwork/muta/commit/2531b8da8e8f2a839484adef62dd93f1deff12dd))



## [0.1.2-beta1](https://github.com/nervosnetwork/muta/compare/v0.1.0-rc.2-huobi...v0.1.2-beta1) (2020-06-01)


### Bug Fixes

* **ci:** Increase timeout in ci ([#262](https://github.com/nervosnetwork/muta/issues/262)) ([a12124a](https://github.com/nervosnetwork/muta/commit/a12124a115512196894a7ca88fc42555db927666))
* **mempool:** check exsit before insert a transaction ([#257](https://github.com/nervosnetwork/muta/issues/257)) ([be3c139](https://github.com/nervosnetwork/muta/commit/be3c13929d2a59f21655b040aa6738c3d43db611))
* **network:** broken users_cast ([#261](https://github.com/nervosnetwork/muta/issues/261)) ([f36eabd](https://github.com/nervosnetwork/muta/commit/f36eabdc5040bc5cbf0d2011c942867150534a41))
* **network:** reconnection fialure ([#273](https://github.com/nervosnetwork/muta/issues/273)) ([9f594b8](https://github.com/nervosnetwork/muta/commit/9f594b8af12e1810bd0cbf23f20ca718d96f6e3a))
* reboot when the diff between height and exec_height more than one ([#267](https://github.com/nervosnetwork/muta/issues/267)) ([e8f8595](https://github.com/nervosnetwork/muta/commit/e8f85958d85e3363fccbfde3971684ebf2fceb4d))
* **sync:** Avoid requesting redundant transactions ([#259](https://github.com/nervosnetwork/muta/issues/259)) ([8ece029](https://github.com/nervosnetwork/muta/commit/8ece0299fe185667ac23fed92d8c2f156c0e2c5b))
* binding store type should return Option None instead of panic when get none ([#238](https://github.com/nervosnetwork/muta/issues/238)) ([54bdbb9](https://github.com/nervosnetwork/muta/commit/54bdbb93df1a1a85a83814dcb29461acf3645d10))
* **config:** use serde(default) for rocksdb conf ([#229](https://github.com/nervosnetwork/muta/issues/229)) ([2a03e73](https://github.com/nervosnetwork/muta/commit/2a03e73c77807e80020c50bb287adf4d428632e5))
* **storage:** fix rocksdb too many open files error ([#228](https://github.com/nervosnetwork/muta/issues/228)) ([96c32cd](https://github.com/nervosnetwork/muta/commit/96c32cd7956220beddca33b22d4663a675573ba9))
* **sync:** set crypto info when synchronization ([#235](https://github.com/nervosnetwork/muta/issues/235)) ([84ccfc1](https://github.com/nervosnetwork/muta/commit/84ccfc1d8422265028ad7a0b460b4e297d161fe3))
* docker compose configs ([#210](https://github.com/nervosnetwork/muta/issues/210)) ([acc5265](https://github.com/nervosnetwork/muta/commit/acc52653d304ac5cd25a9d643b263a2f462f7d43))
* hang when kill it ([#225](https://github.com/nervosnetwork/muta/issues/225)) ([dc51240](https://github.com/nervosnetwork/muta/commit/dc512405f32854f165f3145c01d022bca4fff93b))
* panic when start ([#214](https://github.com/nervosnetwork/muta/issues/214)) ([d2da69b](https://github.com/nervosnetwork/muta/commit/d2da69b5941a88376b64453f7d3c10eca3f67d81))
* **muta:** hangs up on one cpu core ([#203](https://github.com/nervosnetwork/muta/issues/203)) ([555dd9e](https://github.com/nervosnetwork/muta/commit/555dd9e694fda043be01f90c91396efd7fe0ace5))


### Features

* split monitor network url  ([#300](https://github.com/nervosnetwork/muta/issues/300)) ([1237354](https://github.com/nervosnetwork/muta/commit/12373544598d0dae852321cbe3b4e8dab5c70e54))
* supported mempool monitor ([#298](https://github.com/nervosnetwork/muta/issues/298)) ([cc7fdfa](https://github.com/nervosnetwork/muta/commit/cc7fdfa7a7c99466d76d4fe9c1a3537ab8754837))
* supported new metrics ([#294](https://github.com/nervosnetwork/muta/issues/294)) ([e59364a](https://github.com/nervosnetwork/muta/commit/e59364a7759960d8a3279dc78844965f54f4bf62))
* **apm:** add api get_block metrics ([#276](https://github.com/nervosnetwork/muta/issues/276)) ([6ea21e3](https://github.com/nervosnetwork/muta/commit/6ea21e3e0fe08898264f13938cf849c197531afa))
* **apm:** Add opentracing ([#270](https://github.com/nervosnetwork/muta/issues/270)) ([cece21d](https://github.com/nervosnetwork/muta/commit/cece21d8e865223c8679e54d0253ced70dab4c0a))
* **apm:** tracing height and round in OverlordMsg ([#287](https://github.com/nervosnetwork/muta/issues/287)) ([a8c09ff](https://github.com/nervosnetwork/muta/commit/a8c09ff363e8caac9c0977db2fc6cffb782961d7))
* **ci:** add e2e ([#236](https://github.com/nervosnetwork/muta/issues/236)) ([3058722](https://github.com/nervosnetwork/muta/commit/3058722081084b7cb8f423c26eba9e88707fca18))
* **consensus:** add proof check logic for sync and consensus ([#224](https://github.com/nervosnetwork/muta/issues/224)) ([b19502f](https://github.com/nervosnetwork/muta/commit/b19502f48e6d314717a8a2286ada58f6097c6f31))
* **consensus:** change validator list ([#211](https://github.com/nervosnetwork/muta/issues/211)) ([bb04d2c](https://github.com/nervosnetwork/muta/commit/bb04d2c961110276d38cf0e07239d5e72e8125a8))
* **consensus:** integrate trust metric to consensus ([#244](https://github.com/nervosnetwork/muta/issues/244)) ([3dd6bc1](https://github.com/nervosnetwork/muta/commit/3dd6bc1796ca3e6c76cb99beefd5911d35a5e8ee))
* **mempool:** integrate trust metric ([#245](https://github.com/nervosnetwork/muta/issues/245)) ([49474fd](https://github.com/nervosnetwork/muta/commit/49474fddde3ffc45d564544bb5887bb09a37da1d))
* **metric:** introduce metric using prometheus ([#271](https://github.com/nervosnetwork/muta/issues/271)) ([3d1dc4f](https://github.com/nervosnetwork/muta/commit/3d1dc4fcf196b8616f41dc4cd2a5ba0c0a5ab422))
* **metrics:** mempool, consensus and sync ([#275](https://github.com/nervosnetwork/muta/issues/275)) ([12e4918](https://github.com/nervosnetwork/muta/commit/12e4918d9925868407f854af29410d8ecafe4d48))
* **network:** add metrics ([#274](https://github.com/nervosnetwork/muta/issues/274)) ([56a9b62](https://github.com/nervosnetwork/muta/commit/56a9b62251106d44df33c43d4590575df25df61a))
* **network:** add trace header to network msg ([#281](https://github.com/nervosnetwork/muta/issues/281)) ([6509cbe](https://github.com/nervosnetwork/muta/commit/6509cbec2f700238b2259943212e0968b58404ce))
* **network:** peer trust metric ([#231](https://github.com/nervosnetwork/muta/issues/231)) ([5abefeb](https://github.com/nervosnetwork/muta/commit/5abefebddacfb58415f2a319098bb164ceaa8c81))
* add tx hook in framework ([#218](https://github.com/nervosnetwork/muta/issues/218)) ([cdeb9fd](https://github.com/nervosnetwork/muta/commit/cdeb9fd1e18e198636fa59d91aead85d65cf9852))
* re-execute blocks to recover current status ([#222](https://github.com/nervosnetwork/muta/issues/222)) ([1cd7cb6](https://github.com/nervosnetwork/muta/commit/1cd7cb6d4fbc599bac65bd2c36b507088a3fa041))
* **network:** rpc remote server error response ([#205](https://github.com/nervosnetwork/muta/issues/205)) ([bb993ac](https://github.com/nervosnetwork/muta/commit/bb993ac1f5fe44a2f6a72c8718572accacb27dc3))
* **sync:** Split a transaction in a block into multiple requests ([#221](https://github.com/nervosnetwork/muta/issues/221)) ([0bbf43c](https://github.com/nervosnetwork/muta/commit/0bbf43c49d2df49d70b4bc816ac24c3bc3603a1a))
* add actix payload size limit config ([#204](https://github.com/nervosnetwork/muta/issues/204)) ([97319d6](https://github.com/nervosnetwork/muta/commit/97319d6d22c8143ba35c3fe42d56f2cfbc131e37))


### BREAKING CHANGES

* **network:** change rpc response

* change(network): bump transmitter protocol version



# [0.1.0-rc.2-huobi](https://github.com/nervosnetwork/muta/compare/v0.0.1-rc1-huobi...v0.1.0-rc.2-huobi) (2020-02-24)


### Bug Fixes

* **mempool:** fix repeat txs, add flush_incumbent_queue ([#189](https://github.com/nervosnetwork/muta/issues/189)) ([e0db745](https://github.com/nervosnetwork/muta/commit/e0db745419c5ada3d6e9dc4416945a0775a8f18b))
* **muta:** hangs up running on single core environment ([#201](https://github.com/nervosnetwork/muta/issues/201)) ([09f5b4e](https://github.com/nervosnetwork/muta/commit/09f5b4ed70a519155933f7fd4c2015ff512dfdb1))
* block hash from bytes ([#192](https://github.com/nervosnetwork/muta/issues/192)) ([7ca0af4](https://github.com/nervosnetwork/muta/commit/7ca0af46edbd00e4ba43e8646e77fa41aba781cf))


### Features

* check size and cycle limit when insert tx into mempool ([#195](https://github.com/nervosnetwork/muta/issues/195)) ([92bdf2d](https://github.com/nervosnetwork/muta/commit/92bdf2d5147502e1d250fdae47b8ae2c2cfce23f))
* remove redundant wal transactions when commit ([#197](https://github.com/nervosnetwork/muta/issues/197)) ([3aff1db](https://github.com/nervosnetwork/muta/commit/3aff1dbb2dcdabaaf9cbecb9c3e9757a2c737354))
* Supports actix in tokio ([#200](https://github.com/nervosnetwork/muta/issues/200)) ([266c1cb](https://github.com/nervosnetwork/muta/commit/266c1cb2cf6223759eba4ca9771ee21b244db3a4))
* **api:** Supports configuring the max number of connections. ([#194](https://github.com/nervosnetwork/muta/issues/194)) ([6cbdd26](https://github.com/nervosnetwork/muta/commit/6cbdd267b7ff56eefbe23bffc8e4dc589272111d))
* **service:** upgrade asset service ([#150](https://github.com/nervosnetwork/muta/issues/150)) ([8925390](https://github.com/nervosnetwork/muta/commit/8925390b59353d853dd1266cdcfe6db1258a8296))


### Reverts

* Revert "fix(muta): hangs up running on single core environment (#201)" (#202) ([28e685a](https://github.com/nervosnetwork/muta/commit/28e685a62b82c1a91699b4495d430b0757e5438d)), closes [#201](https://github.com/nervosnetwork/muta/issues/201) [#202](https://github.com/nervosnetwork/muta/issues/202)



## [0.0.1-rc1-huobi](https://github.com/nervosnetwork/muta/compare/v0.0.1-rc.1-huobi...v0.0.1-rc1-huobi) (2020-02-15)


### Bug Fixes

* **ci:** fail to install sccache after new rust-toolchain ([#68](https://github.com/nervosnetwork/muta/issues/68)) ([f961415](https://github.com/nervosnetwork/muta/commit/f961415803ae6d38b70e97a810f33a1b60639d43))
* **consensus:** check logs bloom when check block ([#168](https://github.com/nervosnetwork/muta/issues/168)) ([0984989](https://github.com/nervosnetwork/muta/commit/09849893270cc0908e2ee965e7e8b7c46ada0f16))
* **consensus:** empty block receipts root ([#61](https://github.com/nervosnetwork/muta/issues/61)) ([89ed4d2](https://github.com/nervosnetwork/muta/commit/89ed4d2c4a708f278e7cd777c562f1f1fb5a9755))
* **consensus:** encode overlord message and verify signature ([#39](https://github.com/nervosnetwork/muta/issues/39)) ([b11e69e](https://github.com/nervosnetwork/muta/commit/b11e69e49ed195d0d23f22b6abf1387f4a4c0c94))
* **consensus:** fix check state roots ([#107](https://github.com/nervosnetwork/muta/issues/107)) ([cf45c3b](https://github.com/nervosnetwork/muta/commit/cf45c3ba39eb65bdb012165e232352a9187a6f0d))
* **consensus:** Get authority list returns none. ([#4](https://github.com/nervosnetwork/muta/issues/4)) ([2a7eb3c](https://github.com/nervosnetwork/muta/commit/2a7eb3c26fade5a065ec2435b4ba46b6c16f223a))
* **consensus:** state root can not be clear ([#140](https://github.com/nervosnetwork/muta/issues/140)) ([4ea1df4](https://github.com/nervosnetwork/muta/commit/4ea1df425620482f36daf61b4b50edb83807efdd))
* **consensus:** sync txs context no session id ([#167](https://github.com/nervosnetwork/muta/issues/167)) ([53136c3](https://github.com/nervosnetwork/muta/commit/53136c3dfdf0e7b29762cd72f51eeb35d52804c2))
* **doc:** fix graphql_api doc link and doc-api build sh ([#161](https://github.com/nervosnetwork/muta/issues/161)) ([e67e2b2](https://github.com/nervosnetwork/muta/commit/e67e2b24bf0609c263f59381a83fcf04d2227583))
* **executor:** wrong hook logic ([#127](https://github.com/nervosnetwork/muta/issues/127)) ([8c6a246](https://github.com/nervosnetwork/muta/commit/8c6a246a1b64a197371305856148b034320f1fa0))
* **framework/executor:** Catch any errors in the call. ([#92](https://github.com/nervosnetwork/muta/issues/92)) ([739a126](https://github.com/nervosnetwork/muta/commit/739a126c86643b28e1c47aef87d8bd803b9fe8d9))
* **keypair:** Use hex encoding common_ref. ([#79](https://github.com/nervosnetwork/muta/issues/79)) ([abbce4c](https://github.com/nervosnetwork/muta/commit/abbce4c15919f45f824bd4967ea64f8234548765))
* **makefile:** Docker push to the correct image ([#146](https://github.com/nervosnetwork/muta/issues/146)) ([05f6396](https://github.com/nervosnetwork/muta/commit/05f6396f1786b46b4cf9c41e3f700b37ebaddb68))
* **mempool:** Always get the latest epoch id when `package`. ([#30](https://github.com/nervosnetwork/muta/issues/30)) ([9a77ebf](https://github.com/nervosnetwork/muta/commit/9a77ebf9ecba6323cc81cd094774e32fd28b946e))
* **mempool:** broadcast new transactions ([#32](https://github.com/nervosnetwork/muta/issues/32)) ([086ec7e](https://github.com/nervosnetwork/muta/commit/086ec7eb6ca2c8f6afc14767d51efdb91533f932))
* **mempool:** Fix concurrent insert bug of mempool ([#19](https://github.com/nervosnetwork/muta/issues/19)) ([515eec2](https://github.com/nervosnetwork/muta/commit/515eec2ab65a2d57a5ca742c774daeb9cef99354))
* **mempool:** Resize the queue to ensure correct switching. ([#18](https://github.com/nervosnetwork/muta/issues/18)) ([ebf1ae3](https://github.com/nervosnetwork/muta/commit/ebf1ae34861fc48297813cdc465e4d9c99e059d4))
* **mempool:** sync proposal txs doesn't insert txs at all ([#179](https://github.com/nervosnetwork/muta/issues/179)) ([33f39c5](https://github.com/nervosnetwork/muta/commit/33f39c5bac0235a8261c53327c558864a6149c8a))
* **network:** dead lock in peer manager ([#24](https://github.com/nervosnetwork/muta/issues/24)) ([a74017a](https://github.com/nervosnetwork/muta/commit/a74017aa9d84b6b862683860e63c000b4048e459))
* **network:** default rpc timeout to 4 seconds ([#115](https://github.com/nervosnetwork/muta/issues/115)) ([666049c](https://github.com/nervosnetwork/muta/commit/666049c54c8eee8291cc173230caccb35de137ca))
* **network:** fail to bootstrap if bootstrap isn't start already ([#46](https://github.com/nervosnetwork/muta/issues/46)) ([9dd515a](https://github.com/nervosnetwork/muta/commit/9dd515a3e09f1c158dff6536ed38eb5116f4317f))
* **network:** give up retry ([#152](https://github.com/nervosnetwork/muta/issues/152)) ([34d052a](https://github.com/nervosnetwork/muta/commit/34d052aaba1684333fdd49f86e54c103064fa2f6))
* **network:** never reconnect bootstrap again after failure ([#22](https://github.com/nervosnetwork/muta/issues/22)) ([79d66bd](https://github.com/nervosnetwork/muta/commit/79d66bd06e61ff6ef41c12ada91cf6485482aa43))
* **network:** NoSessionId Error ([#33](https://github.com/nervosnetwork/muta/issues/33)) ([4761d79](https://github.com/nervosnetwork/muta/commit/4761d797dded9534e0c0b5e43c6e519055542c2c))
* **network:** rpc memory leak if rpc call future is dropped ([#166](https://github.com/nervosnetwork/muta/issues/166)) ([8476a4b](https://github.com/nervosnetwork/muta/commit/8476a4b85bf3cf923adcd7555cef04ae73a225f1))
* **sync:** Check the height again after get the lock ([#171](https://github.com/nervosnetwork/muta/issues/171)) ([68164f3](https://github.com/nervosnetwork/muta/commit/68164f3f75d83b9507ee68a099fb712492339edb))
* **sync:** Flush the memory pool when the storage success ([#165](https://github.com/nervosnetwork/muta/issues/165)) ([3b9cbd5](https://github.com/nervosnetwork/muta/commit/3b9cbd55310993c783b0a5794237df75accf118e))
* fix overlord not found error ([#95](https://github.com/nervosnetwork/muta/issues/95)) ([0754c64](https://github.com/nervosnetwork/muta/commit/0754c64973f7fca92e49080c3a03a869b43a4c46))
* Ignore bootstraps when empty. ([#41](https://github.com/nervosnetwork/muta/issues/41)) ([2b3566b](https://github.com/nervosnetwork/muta/commit/2b3566b4acb91f6086b9cca2b1ea4d2883a75be9))


### Features

* **config:** move bls_pub_key config to genesis.toml ([#162](https://github.com/nervosnetwork/muta/issues/162)) ([337b01f](https://github.com/nervosnetwork/muta/commit/337b01fda21fc33f4d4817d93a27d86af9e2b164))
* **network:** interval report pending data size ([#160](https://github.com/nervosnetwork/muta/issues/160)) ([3c46aca](https://github.com/nervosnetwork/muta/commit/3c46aca4873abf9b8afd01d5f464df57bb1b9b9a))
* **sync:** Trigger sync after waiting for consensus interval ([#169](https://github.com/nervosnetwork/muta/issues/169)) ([fe355f1](https://github.com/nervosnetwork/muta/commit/fe355f1d7d6359dfa97809f1bc603cb99975ba46))
* add api schema ([#90](https://github.com/nervosnetwork/muta/issues/90)) ([3f8adfa](https://github.com/nervosnetwork/muta/commit/3f8adfa0a717b055a4455fd102de68003f835bf2))
* add common_ref argument for keypair tool ([#154](https://github.com/nervosnetwork/muta/issues/154)) ([2651346](https://github.com/nervosnetwork/muta/commit/26513469206aa8a4480c5fffad9d134d5d0e8ded))
* add panic hook to logger ([#156](https://github.com/nervosnetwork/muta/issues/156)) ([93b65fe](https://github.com/nervosnetwork/muta/commit/93b65feb89502b7d7836d7f4c423db37fbd1ef4f))
* Extract muta as crate. ([1b62fe7](https://github.com/nervosnetwork/muta/commit/1b62fe786fbd576b67ea28df3d304d235ae3e94e))
* Metadata service ([#133](https://github.com/nervosnetwork/muta/issues/133)) ([a588b12](https://github.com/nervosnetwork/muta/commit/a588b12de4f3c0de666b66e2a5dea65d71977f5f))
* spawn sync txs in check epoch ([6dca1dd](https://github.com/nervosnetwork/muta/commit/6dca1ddcd9256a3061f132a5abc5d784d466c168))
* support specify module log level via config ([#105](https://github.com/nervosnetwork/muta/issues/105)) ([c06061b](https://github.com/nervosnetwork/muta/commit/c06061b4ccd755177385dfee000783e2b11b0dcd))
* Update juniper, supports async ([#149](https://github.com/nervosnetwork/muta/issues/149)) ([cbabf50](https://github.com/nervosnetwork/muta/commit/cbabf507c25ee8feb8a57de408bc97efc8a4a4ab))
* update overlord with brake engine ([#159](https://github.com/nervosnetwork/muta/issues/159)) ([8cd886a](https://github.com/nervosnetwork/muta/commit/8cd886a79fec934a53d409a27de941f16166c176)), closes [#156](https://github.com/nervosnetwork/muta/issues/156) [#158](https://github.com/nervosnetwork/muta/issues/158)
* **api:** Add the exec_height field to the block ([#138](https://github.com/nervosnetwork/muta/issues/138)) ([417153c](https://github.com/nervosnetwork/muta/commit/417153c632793c7ac4e7bc3ffa5b2832dd2dbe66))
* **binding-macro:** service method supports none payload and none response ([#103](https://github.com/nervosnetwork/muta/issues/103)) ([3a5783e](https://github.com/nervosnetwork/muta/commit/3a5783eadd1090cf739d4fdbe94f049115eb65f0))
* **consensus:** develop aggregate crypto with overlord ([#60](https://github.com/nervosnetwork/muta/issues/60)) ([2bc0869](https://github.com/nervosnetwork/muta/commit/2bc0869e928b35c674b4cafdf48540298752b5b5))
* **core/binding:** Implementation of service state. ([#48](https://github.com/nervosnetwork/muta/issues/48)) ([301be6f](https://github.com/nervosnetwork/muta/commit/301be6f39379bd3826b5f605c999ce107f7404e4))
* **core/binding-macro:** Add `read` and `write` proc-macro. ([#49](https://github.com/nervosnetwork/muta/issues/49)) ([687b6e1](https://github.com/nervosnetwork/muta/commit/687b6e1e1a960f679394843c42b861981828d8aa))
* **core/binding-macro:** Add cycles proc-marco. ([#52](https://github.com/nervosnetwork/muta/issues/52)) ([e2289a2](https://github.com/nervosnetwork/muta/commit/e2289a2481510b59c18e37d0fc8bedd9f5d4537e))
* **core/binding-macro:** Support for returning a struct. ([#70](https://github.com/nervosnetwork/muta/issues/70)) ([e13b1ff](https://github.com/nervosnetwork/muta/commit/e13b1ff7834279de9c2df5a0df6967035b7fb8b3))
* **framework:** add ExecutorParams into hook method ([#116](https://github.com/nervosnetwork/muta/issues/116)) ([8036bd6](https://github.com/nervosnetwork/muta/commit/8036bd6f9be1f49eedbc40bbc260ad82952c2e71))
* **framework:** add extra: Option<Bytes> to ServiceContext ([#118](https://github.com/nervosnetwork/muta/issues/118)) ([694c4a3](https://github.com/nervosnetwork/muta/commit/694c4a34f32dc1ba4940db19e304de7a927e1531))
* **framework:** add tx_hash, nonce to ServiceContext ([#111](https://github.com/nervosnetwork/muta/issues/111)) ([352f71f](https://github.com/nervosnetwork/muta/commit/352f71fb3b8b024d533d26c7a344fad801b7a91c))
* **framework/executor:** create service genesis from config ([#104](https://github.com/nervosnetwork/muta/issues/104)) ([8988ccb](https://github.com/nervosnetwork/muta/commit/8988ccb3e5cb2a25bfeabe93c5a63ac1600290a2))
* **graphql:** Modify the API to fit the framework data structure. ([#74](https://github.com/nervosnetwork/muta/issues/74)) ([a1ca2b0](https://github.com/nervosnetwork/muta/commit/a1ca2b0d68e32e335d8d388b70bca83137519f5a))
* **muta:** flush metadata while commit  ([#137](https://github.com/nervosnetwork/muta/issues/137)) ([383a481](https://github.com/nervosnetwork/muta/commit/383a481c348efdf73fd690b42b2430fca6d9a0db))
* **muta:** link up metadata service with muta ([#136](https://github.com/nervosnetwork/muta/issues/136)) ([ba65b80](https://github.com/nervosnetwork/muta/commit/ba65b80dffd128f12336b44d4e80ed40cced8e75))
* **protocol/traits:** Add traits of binding. ([#47](https://github.com/nervosnetwork/muta/issues/47)) ([c6b85ee](https://github.com/nervosnetwork/muta/commit/c6b85ee7bee5b14c5da1676ff44d743c031a0fa6))
* **protocol/types:** Add cycles_price for raw_transaction. ([#46](https://github.com/nervosnetwork/muta/issues/46)) ([55f64a4](https://github.com/nervosnetwork/muta/commit/55f64a49634061ca05c75cbf5923f183fc83936d))
* **sync:** Wait for the execution queue. ([#132](https://github.com/nervosnetwork/muta/issues/132)) ([a8d2013](https://github.com/nervosnetwork/muta/commit/a8d2013991cc6b5b579429954c8411c7954b1da4))
* add end to end test ([#42](https://github.com/nervosnetwork/muta/issues/42)) ([e84756d](https://github.com/nervosnetwork/muta/commit/e84756d1734ad58943309c3c2299393f5a2022e4))
* Extract muta as crate. ([#75](https://github.com/nervosnetwork/muta/issues/75)) ([fc576ea](https://github.com/nervosnetwork/muta/commit/fc576eaa67a3b4b4fa459b0ab970251d63b06b4f)), closes [#46](https://github.com/nervosnetwork/muta/issues/46) [#47](https://github.com/nervosnetwork/muta/issues/47) [#48](https://github.com/nervosnetwork/muta/issues/48) [#49](https://github.com/nervosnetwork/muta/issues/49) [#52](https://github.com/nervosnetwork/muta/issues/52) [#51](https://github.com/nervosnetwork/muta/issues/51) [#55](https://github.com/nervosnetwork/muta/issues/55) [#58](https://github.com/nervosnetwork/muta/issues/58) [#56](https://github.com/nervosnetwork/muta/issues/56) [#64](https://github.com/nervosnetwork/muta/issues/64) [#65](https://github.com/nervosnetwork/muta/issues/65) [#70](https://github.com/nervosnetwork/muta/issues/70) [#71](https://github.com/nervosnetwork/muta/issues/71) [#72](https://github.com/nervosnetwork/muta/issues/72) [#73](https://github.com/nervosnetwork/muta/issues/73) [#43](https://github.com/nervosnetwork/muta/issues/43) [#54](https://github.com/nervosnetwork/muta/issues/54) [#53](https://github.com/nervosnetwork/muta/issues/53) [#57](https://github.com/nervosnetwork/muta/issues/57) [#45](https://github.com/nervosnetwork/muta/issues/45) [#62](https://github.com/nervosnetwork/muta/issues/62) [#63](https://github.com/nervosnetwork/muta/issues/63) [#66](https://github.com/nervosnetwork/muta/issues/66) [#61](https://github.com/nervosnetwork/muta/issues/61) [#67](https://github.com/nervosnetwork/muta/issues/67) [#68](https://github.com/nervosnetwork/muta/issues/68) [#60](https://github.com/nervosnetwork/muta/issues/60) [#46](https://github.com/nervosnetwork/muta/issues/46) [#47](https://github.com/nervosnetwork/muta/issues/47) [#48](https://github.com/nervosnetwork/muta/issues/48) [#49](https://github.com/nervosnetwork/muta/issues/49) [#52](https://github.com/nervosnetwork/muta/issues/52) [#51](https://github.com/nervosnetwork/muta/issues/51) [#55](https://github.com/nervosnetwork/muta/issues/55) [#58](https://github.com/nervosnetwork/muta/issues/58) [#56](https://github.com/nervosnetwork/muta/issues/56) [#64](https://github.com/nervosnetwork/muta/issues/64) [#65](https://github.com/nervosnetwork/muta/issues/65) [#70](https://github.com/nervosnetwork/muta/issues/70) [#72](https://github.com/nervosnetwork/muta/issues/72) [#74](https://github.com/nervosnetwork/muta/issues/74)
* metrics logger ([#43](https://github.com/nervosnetwork/muta/issues/43)) ([d633309](https://github.com/nervosnetwork/muta/commit/d6333091959da6ab0a12630282f6ea783d509319))
* support consensus tracing ([#53](https://github.com/nervosnetwork/muta/issues/53)) ([03942f0](https://github.com/nervosnetwork/muta/commit/03942f08cfdcc573d7feef3a1111e59f63d077f1))
* **api:** make API more user-friendly ([#38](https://github.com/nervosnetwork/muta/issues/38)) ([ba33467](https://github.com/nervosnetwork/muta/commit/ba33467e52c114576b82850e11662d168ede293a))
* **mempool:** implement cached batch txs broadcast ([#20](https://github.com/nervosnetwork/muta/issues/20)) ([d2af811](https://github.com/nervosnetwork/muta/commit/d2af811bb99becc9600d784ce19e021fec11627d))
* **sync:** synchronization epoch ([#9](https://github.com/nervosnetwork/muta/issues/9)) ([fb4bf0d](https://github.com/nervosnetwork/muta/commit/fb4bf0d7c4bde7c86d1b09f469037ff1219f15fa)), closes [#17](https://github.com/nervosnetwork/muta/issues/17) [#18](https://github.com/nervosnetwork/muta/issues/18)
* add compile and run in README ([#11](https://github.com/nervosnetwork/muta/issues/11)) ([1058322](https://github.com/nervosnetwork/muta/commit/10583224053ab91c32dbec815cd0a5af6b0dbeb3))
* add docker ([#31](https://github.com/nervosnetwork/muta/issues/31)) ([8a4386a](https://github.com/nervosnetwork/muta/commit/8a4386ad4c1f66783cada885db9851609b6f5f8d))
* change rlp in executor to fixed-codec ([#29](https://github.com/nervosnetwork/muta/issues/29)) ([7f737cd](https://github.com/nervosnetwork/muta/commit/7f737cdfc9353148b945ad52dd5ab3fd46e2c4db))
* Get balance. ([#28](https://github.com/nervosnetwork/muta/issues/28)) ([8c4a3f9](https://github.com/nervosnetwork/muta/commit/8c4a3f9af8b9e1e8f19cc50b280b66b5d8e270bb))
* **codec:** Add codec tests and benchmarks ([#22](https://github.com/nervosnetwork/muta/issues/22)) ([dcbe522](https://github.com/nervosnetwork/muta/commit/dcbe522be22596059280f6ef845a6d6f4e798551))
* **consensus:** develop consensus interfaces ([#21](https://github.com/nervosnetwork/muta/issues/21)) ([62e3c06](https://github.com/nervosnetwork/muta/commit/62e3c063cd4f82efda43ca5c87c042db5adb9abb))
* **consensus:** develop consensus provider and engine ([#28](https://github.com/nervosnetwork/muta/issues/28)) ([b2ccf9c](https://github.com/nervosnetwork/muta/commit/b2ccf9c84502a6dd476b1737aa9cbb2a283ced32))
* **consensus:** Execute the transactions on commit. ([#7](https://github.com/nervosnetwork/muta/issues/7)) ([b54e7d2](https://github.com/nervosnetwork/muta/commit/b54e7d2bbd5d0ac45ef0d4c728e398b87a1f5450))
* **consensus:** joint overlord and chain ([#32](https://github.com/nervosnetwork/muta/issues/32)) ([72cec41](https://github.com/nervosnetwork/muta/commit/72cec41c86824455ad35cfb1da8a246c50731568))
* **consensus:** mutex lock and timer config ([#45](https://github.com/nervosnetwork/muta/issues/45)) ([cf09687](https://github.com/nervosnetwork/muta/commit/cf09687299b5be39a9c40f13d4b88a496ec7c943))
* **consensus:** Support trsanction executor. ([#6](https://github.com/nervosnetwork/muta/issues/6)) ([e1188f9](https://github.com/nervosnetwork/muta/commit/e1188f9296b3947f833d6bc9a9beff22ebbbf4e7))
* **executor:** Create genesis. ([#1](https://github.com/nervosnetwork/muta/issues/1)) ([a1111d8](https://github.com/nervosnetwork/muta/commit/a1111d8db709c62d119edf3238a22dd656e8035f))
* **graphql:** Support transfer and contract deployment ([#44](https://github.com/nervosnetwork/muta/issues/44)) ([bfcb520](https://github.com/nervosnetwork/muta/commit/bfcb5203fe245e364922d5d8966197a8a8f8d91c))
* **mempool:** fix fixed_codec ([#25](https://github.com/nervosnetwork/muta/issues/25)) ([c1ac607](https://github.com/nervosnetwork/muta/commit/c1ac607ac9b61f4867c17f69c50dad9797dc1c2b))
* **mempool:** Remove cycle_limit ([#23](https://github.com/nervosnetwork/muta/issues/23)) ([8a19ae8](https://github.com/nervosnetwork/muta/commit/8a19ae867fd5b82c4fd56a1f8b59a83e24ca5bc0))
* **native-contract:** Support for asset creation and transfer. ([#37](https://github.com/nervosnetwork/muta/issues/37)) ([1c505fb](https://github.com/nervosnetwork/muta/commit/1c505fbdd57fcb2ef3df3e8b19c65599d77c9bf1))
* **network:** log connected peer ips ([#23](https://github.com/nervosnetwork/muta/issues/23)) ([1691bfa](https://github.com/nervosnetwork/muta/commit/1691bfa47ac561a2f27243e21b1b2fad2fb64be9))
* develop merkle root ([#17](https://github.com/nervosnetwork/muta/issues/17)) ([03cec31](https://github.com/nervosnetwork/muta/commit/03cec318645ee49158f09ec59e356210a80f8bbf))
* Fill in the main function ([#36](https://github.com/nervosnetwork/muta/issues/36)) ([d783f3b](https://github.com/nervosnetwork/muta/commit/d783f3b2d36507a695abd47b303b6c0108e2030b))
* **mempool:** Develop mempool's tests and benches  ([#9](https://github.com/nervosnetwork/muta/issues/9)) ([5ddd5f4](https://github.com/nervosnetwork/muta/commit/5ddd5f4d0c1fa9630971ade538dcf954b6aa8f54))
* **mempool:** Implement MemPool interfaces ([#8](https://github.com/nervosnetwork/muta/issues/8)) ([934ce58](https://github.com/nervosnetwork/muta/commit/934ce58b7a7a6b89b65ff931ce5487e553dd927d))
* **native_contract:** Add an adapter that provides access to the world state. ([#27](https://github.com/nervosnetwork/muta/issues/27)) ([3281bea](https://github.com/nervosnetwork/muta/commit/3281beab2d054470b5edf330515df933cc713bb8))
* **protocol:** Add the mempool traits ([#7](https://github.com/nervosnetwork/muta/issues/7)) ([9f6c19b](https://github.com/nervosnetwork/muta/commit/9f6c19bbfbff6c8f82bb732c3503d757833f837e))
* **protocol:** Add the underlying data structure. ([#5](https://github.com/nervosnetwork/muta/issues/5)) ([5dae189](https://github.com/nervosnetwork/muta/commit/5dae189104c986348adddd43fbaa47af01781828))
* **protocol:** Protobuf serialize ([#6](https://github.com/nervosnetwork/muta/issues/6)) ([ff00595](https://github.com/nervosnetwork/muta/commit/ff00595d100e44148b1cc243437798db8233ca2b))
* **storage:** add storage test ([#18](https://github.com/nervosnetwork/muta/issues/18)) ([f78df5b](https://github.com/nervosnetwork/muta/commit/f78df5b0357eade7855152eee9c79070866477ac))
* **storage:** Implement memory adapter API ([#11](https://github.com/nervosnetwork/muta/issues/11)) ([b0a8090](https://github.com/nervosnetwork/muta/commit/b0a80901229f85e8cf89bd806dcb32c95ae059b8))
* **storage:** Implement storage ([#17](https://github.com/nervosnetwork/muta/issues/17)) ([7728b5b](https://github.com/nervosnetwork/muta/commit/7728b5b0307bd58b11671f123f37e3e365b14b97))
* **types:** Add account structure. ([#24](https://github.com/nervosnetwork/muta/issues/24)) ([f6b93f0](https://github.com/nervosnetwork/muta/commit/f6b93f0f08b03a20761aef47f08343eb5d8e6a85))


### Performance Improvements

* **storage:** cache latest epoch ([#128](https://github.com/nervosnetwork/muta/issues/128)) ([da4d7a9](https://github.com/nervosnetwork/muta/commit/da4d7a92363596b7339518e24c64ab49648749dd))


### Reverts

* Revert "[ᚬdebug-muta] feat(service): Upgrade asset (#181)" (#182) ([dad3f99](https://github.com/nervosnetwork/muta/commit/dad3f99f7c694eea57b546c6b2169950c5692ea1)), closes [#181](https://github.com/nervosnetwork/muta/issues/181) [#182](https://github.com/nervosnetwork/muta/issues/182)
* Revert "feat: Extract muta as crate. (#75)" (#77) ([3baacc5](https://github.com/nervosnetwork/muta/commit/3baacc5c781615377e9a6ba50cfc7b17dcb0ec6e)), closes [#75](https://github.com/nervosnetwork/muta/issues/75) [#77](https://github.com/nervosnetwork/muta/issues/77)



# [0.1.0](https://github.com/nervosnetwork/muta/compare/733ee8e6be7649c9aa2d772bb1dc661bd0879917...v0.1.0) (2019-09-22)


### Bug Fixes

* **ci:** build on push and pull request ([d28aa55](https://github.com/nervosnetwork/muta/commit/d28aa55f5df240277e2b75e87aa948cdcf11ea7f))
* **ci:** temporarily amend code to pass lint ([9441236](https://github.com/nervosnetwork/muta/commit/9441236a5107e0042753915ed943b487cd02d6a5))
* **consensus:** Clear cache of last proposal. ([#199](https://github.com/nervosnetwork/muta/issues/199)) ([f548653](https://github.com/nervosnetwork/muta/commit/f5486531f43fa720171941ad4be5ec7646a269c2))
* **consensus:** fix lock free too early problem and add state root check ([#277](https://github.com/nervosnetwork/muta/issues/277)) ([7238c5b](https://github.com/nervosnetwork/muta/commit/7238c5bc057bd6c6f31773fa4bd3e06aaea72255))
* **consensus:** Makes sure that proposer is this node. ([#281](https://github.com/nervosnetwork/muta/issues/281)) ([d7f4e50](https://github.com/nervosnetwork/muta/commit/d7f4e5081f00a04aee934d0ce700cd107f4f345f))
* **core-network:** CallbackItemNotFound ([#243](https://github.com/nervosnetwork/muta/issues/243)) ([47365fa](https://github.com/nervosnetwork/muta/commit/47365faf5fa7171dde8951661fa095a6c43bcb1f))
* **core-network:** false bootstrapped connections ([#275](https://github.com/nervosnetwork/muta/issues/275)) ([26e76f0](https://github.com/nervosnetwork/muta/commit/26e76f0a2879aed3da745529f64ba3828a1cc30e))
* **core-types:** compilation failure ([#269](https://github.com/nervosnetwork/muta/issues/269)) ([56d8649](https://github.com/nervosnetwork/muta/commit/56d86491f69ab16fd2c76b66b28ad76df78c6ca7))
* **core/crypto:** pubkey_to_address() consistent with cita ([acb5e63](https://github.com/nervosnetwork/muta/commit/acb5e63ea577429bc94c16a3430035ea139aaf15))
* **executor:** Save the full node data. ([b57a1c5](https://github.com/nervosnetwork/muta/commit/b57a1c5fa775479b85d1531f7d2dced817de4729))
* **jsonrpc:** give default value for newFilter ([#289](https://github.com/nervosnetwork/muta/issues/289)) ([17069b4](https://github.com/nervosnetwork/muta/commit/17069b49067dd7335f243d248e3c8d633e455a73))
* **jsonrpc:** logic error in getTransactionCount ([#290](https://github.com/nervosnetwork/muta/issues/290)) ([464bfdf](https://github.com/nervosnetwork/muta/commit/464bfdf08a9954206bb595b3861c52208fc9630d))
* **jsonrpc:** make the response compatible with jsonrpc 2.0 spec ([1db5190](https://github.com/nervosnetwork/muta/commit/1db5190bc91d431bacce6bb44a1185b19520c1a2))
* **jsonrpc:** prefix with 0x by API getTransactionProof ([#295](https://github.com/nervosnetwork/muta/issues/295)) ([b1c0160](https://github.com/nervosnetwork/muta/commit/b1c0160b65fc91e8a2bcfd908943fb238d1101c1))
* **jsonrpc:** raise error when key not found in state ([#294](https://github.com/nervosnetwork/muta/issues/294)) ([7a7c294](https://github.com/nervosnetwork/muta/commit/7a7c294df5ae75f50ec0fe3620634c7280f837e7))
* **jsonrpc:** returns the correct block hash ([#280](https://github.com/nervosnetwork/muta/issues/280)) ([f6a58d0](https://github.com/nervosnetwork/muta/commit/f6a58d0cfc743d1fa84fe5de99798157ba5f25a6))
* Call header.hash ([#94](https://github.com/nervosnetwork/muta/issues/94)) ([636aa54](https://github.com/nervosnetwork/muta/commit/636aa549c21a04611b6f4575dfc7e78fa47d768e))
* change the blocking thread from rayon to std::thread ([5b80476](https://github.com/nervosnetwork/muta/commit/5b804765d0a76055e6e730560a6d7ecd576703be))
* return err if tx not found in get_batch to avoid forking ([#279](https://github.com/nervosnetwork/muta/issues/279)) ([6aed2fe](https://github.com/nervosnetwork/muta/commit/6aed2fe5ffcd0eb6a699cff00d92e9dd3ab7d7b3))
* **sync:** proof and proposal_hash hash not match. ([#239](https://github.com/nervosnetwork/muta/issues/239)) ([51f332e](https://github.com/nervosnetwork/muta/commit/51f332ee8c4a10b88844a272bc51a116b4d25dd2))
* tokio::spawn panic. ([#238](https://github.com/nervosnetwork/muta/issues/238)) ([12d8d01](https://github.com/nervosnetwork/muta/commit/12d8d01ed42f9cc5d9cc341edfd76a6076aa37e1))
* **common/logger:** cargo fmt ([e3a7f5a](https://github.com/nervosnetwork/muta/commit/e3a7f5a2217956b86191881caeb3ca6cea7ec2fc))
* **compoents/transaction-pool:** Use the latest crypto API. ([#86](https://github.com/nervosnetwork/muta/issues/86)) ([f6c94d3](https://github.com/nervosnetwork/muta/commit/f6c94d307d6e89afba75ed8b83b99088fc7ca9de))
* **components/transaction-pool:** Check if the transaction is repeated in histories block. ([dba25fe](https://github.com/nervosnetwork/muta/commit/dba25fe09d8e82f0e396415055ce08efbf1fe159))
* **core-p2p:** transmission example: a clippy warning ([6d2f42a](https://github.com/nervosnetwork/muta/commit/6d2f42ae97194333a823581406fc75d2c47536b2))
* **core-p2p:** transmission example: remove unreachable match branch ([0082bd6](https://github.com/nervosnetwork/muta/commit/0082bd6a3fb956f9ee17a9eba6ada77fc91f3dfe))
* **core-p2p:** transmission: future task starvation ([ba14db0](https://github.com/nervosnetwork/muta/commit/ba14db035413220ed7eba5e5543b8a6496267641))
* **devchain:** correct addresses matched with privkey ([#114](https://github.com/nervosnetwork/muta/issues/114)) ([f56744e](https://github.com/nervosnetwork/muta/commit/f56744e7809b39da79434a3fbcf3deb127fded27))
* **network:** RepeatedConnection and ConnectSelf errors ([#196](https://github.com/nervosnetwork/muta/issues/196)) ([2e5e888](https://github.com/nervosnetwork/muta/commit/2e5e888cdb0869e7622639919b12e62eca06f137))
* **p2p:** Make sure the "poll" is triggered. ([#182](https://github.com/nervosnetwork/muta/issues/182)) ([88daed1](https://github.com/nervosnetwork/muta/commit/88daed1e3e175c21e7923ddd5f1b4eb4ef4d6286))
* **p2p-identify:** empty local listen addresses ([#198](https://github.com/nervosnetwork/muta/issues/198)) ([c40ad8a](https://github.com/nervosnetwork/muta/commit/c40ad8a8dedd999efd17a88b9c30b198d4a0035a))
* **synchronizer:** add a pull_txs_sync method to sync txs from block ([#207](https://github.com/nervosnetwork/muta/issues/207)) ([317fca8](https://github.com/nervosnetwork/muta/commit/317fca8b8d2f270e5d140a94bb1a9227c4b7271b))
* **transaction-pool:** duplicate insertion transactions from network ([#191](https://github.com/nervosnetwork/muta/issues/191)) ([2c095bb](https://github.com/nervosnetwork/muta/commit/2c095bbe5649454abf2663df7355c0a56f54a71f))
* **tx-pool:** "get_count" returns the repeat transaction. ([f5612d0](https://github.com/nervosnetwork/muta/commit/f5612d09d02e9183b702f0233aecc14c31779945))
* **tx-pool:** `ensure` method always pull all txs from remote peer ([#194](https://github.com/nervosnetwork/muta/issues/194)) ([9ff300e](https://github.com/nervosnetwork/muta/commit/9ff300e191aa39b6301e481f8f287287b645ba39))
* **tx-pool:** Ensure the number of transactions meets expectations ([dcbf0dd](https://github.com/nervosnetwork/muta/commit/dcbf0dd8cf548ddfe3afb3226d7596637ae615dd))
* **tx-pool:** replace chashmap ([#211](https://github.com/nervosnetwork/muta/issues/211)) ([717f55e](https://github.com/nervosnetwork/muta/commit/717f55e4772c5818ab17e2b1c320b0b98f174122))
* Aviod drop ([4d0f986](https://github.com/nervosnetwork/muta/commit/4d0f986741c392489893f036989db7218db54743))
* build failure ([18ce8e4](https://github.com/nervosnetwork/muta/commit/18ce8e4642d8d27892fee53b9695e4ced7921055))
* jsonrpc call return value ([#104](https://github.com/nervosnetwork/muta/issues/104)) ([1fe41eb](https://github.com/nervosnetwork/muta/commit/1fe41eb491a16588019218144985eec143613c65))
* logic error of bloom filter ([#176](https://github.com/nervosnetwork/muta/issues/176)) ([70269cb](https://github.com/nervosnetwork/muta/commit/70269cb5cefd82f1a14eb5e85df419c1587d19c8))
* merkle typo ([4f63585](https://github.com/nervosnetwork/muta/commit/4f6358565ee8d486be18ac8ff6069b95b597ea4d))
* rlp encode ([b852ac1](https://github.com/nervosnetwork/muta/commit/b852ac147db818cf289b972f054028d293218a19))
* rlp hash ([837055a](https://github.com/nervosnetwork/muta/commit/837055a4eb78ba941004dbc0466955895de8bcab))
* Set quota limit for the genesis. ([#106](https://github.com/nervosnetwork/muta/issues/106)) ([931fe40](https://github.com/nervosnetwork/muta/commit/931fe404453a6f936cbd27bf37d0e326a03e4484))
* write lock ([de80439](https://github.com/nervosnetwork/muta/commit/de80439cb4e7889c1220fc7821604f9ef792422e))


### Features

* add business model support for executor ([#308](https://github.com/nervosnetwork/muta/issues/308)) ([e03396b](https://github.com/nervosnetwork/muta/commit/e03396bb6b964a0c93f43c2684a0e76a55db5540))
* add Deserialize for Hash and Address ([#259](https://github.com/nervosnetwork/muta/issues/259)) ([fef188c](https://github.com/nervosnetwork/muta/commit/fef188c5950fb7f64a92312894efdb4955201a93))
* add docker config for dev ([#197](https://github.com/nervosnetwork/muta/issues/197)) ([6e74aec](https://github.com/nervosnetwork/muta/commit/6e74aec0b51c2bf80c1d1b893130ea74f4a1a8f0))
* add fabric devops scripts ([fcdc25c](https://github.com/nervosnetwork/muta/commit/fcdc25c05b5c30ba38bf6af57885c2f45233d3fc))
* add height to the end of proposal msg ([#255](https://github.com/nervosnetwork/muta/issues/255)) ([c5cbc5e](https://github.com/nervosnetwork/muta/commit/c5cbc5ec70f1dc0fb46ef0bb87c3b994596b4571))
* add more info to version ([#298](https://github.com/nervosnetwork/muta/issues/298)) ([fd02a17](https://github.com/nervosnetwork/muta/commit/fd02a17a68bb6ef59bbd4cded13d69da221237ee))
* peerCount RPC API ([#257](https://github.com/nervosnetwork/muta/issues/257)) ([736ae8c](https://github.com/nervosnetwork/muta/commit/736ae8c7f537a56b01d648cf066f220e47108820))
* **components/cita-jsonrpc:** impl executor related apis ([#80](https://github.com/nervosnetwork/muta/issues/80)) ([bc8f340](https://github.com/nervosnetwork/muta/commit/bc8f34015617e1a01fb2fbb30d9709cdd806daea))
* **components/cita-jsonrpc:** impl get_code and finish some todo ([#87](https://github.com/nervosnetwork/muta/issues/87)) ([e1b0b9d](https://github.com/nervosnetwork/muta/commit/e1b0b9dc8c39965366c5b572905e63cacecdc958))
* **components/databse:** Implement RocksDB ([#72](https://github.com/nervosnetwork/muta/issues/72)) ([3516fbc](https://github.com/nervosnetwork/muta/commit/3516fbc41338a2f423e0ba56eb96c7fa697a6c77))
* **components/executor:** Add trie db for executor. ([#85](https://github.com/nervosnetwork/muta/issues/85)) ([fd7dc1d](https://github.com/nervosnetwork/muta/commit/fd7dc1da97a4b7dafb1ecbc2813c9506423689a5))
* **components/executor:** Implement EVM executor. ([#68](https://github.com/nervosnetwork/muta/issues/68)) ([021893d](https://github.com/nervosnetwork/muta/commit/021893db432f1ddadc89da9c9251bdb6fb79d925))
* **components/jsonrpc:** implement getStateProof ([#178](https://github.com/nervosnetwork/muta/issues/178)) ([69499fb](https://github.com/nervosnetwork/muta/commit/69499fbb98cbe7f23d426c15ebe67de552dd5d2b))
* **components/jsonrpc:** implement getTransactionProof ([0db8785](https://github.com/nervosnetwork/muta/commit/0db8785475e9d9c098fa123b9c23b4f0eab286dc))
* **components/jsonrpc:** running on microscope ([#200](https://github.com/nervosnetwork/muta/issues/200)) ([1c63a0e](https://github.com/nervosnetwork/muta/commit/1c63a0e3db751b7b7be6f053bed2b66245b105cd))
* **components/jsonrpc:** Try to convert tx to cita::tx ([#221](https://github.com/nervosnetwork/muta/issues/221)) ([b8ab16b](https://github.com/nervosnetwork/muta/commit/b8ab16b05ad01a0c6ef5a7b8d7ad76961e7749ff))
* **core-network:** expost send_buffer_size and recv_buffer_size ([#248](https://github.com/nervosnetwork/muta/issues/248)) ([e5120ad](https://github.com/nervosnetwork/muta/commit/e5120ad646c9d206b43b0d50911303507bdfe381))
* **core-network:** implement peer count feature ([#256](https://github.com/nervosnetwork/muta/issues/256)) ([8f7e7eb](https://github.com/nervosnetwork/muta/commit/8f7e7eb51cdeebfb9c679d88626ac2ec3fa651a4))
* add performance test lua script ([#244](https://github.com/nervosnetwork/muta/issues/244)) ([c727b73](https://github.com/nervosnetwork/muta/commit/c727b733340029f72d9280a57e07522f635eff44))
* **core-network:** implement concurrent reactor and real chained reactor ([#175](https://github.com/nervosnetwork/muta/issues/175)) ([dc9f897](https://github.com/nervosnetwork/muta/commit/dc9f897f08801d7b8a418750ed516a8acac057ca))
* **core-p2p:** implement datagram transport protocol ([fee2d45](https://github.com/nervosnetwork/muta/commit/fee2d4546552bd6c46376309eb399126219c55fb))
* **core-p2p:** transmission: use `poll` func to do broadcast ([b376cbe](https://github.com/nervosnetwork/muta/commit/b376cbef9211e55f809f16bb9bab1360dd4b3523))
* **core/consensus:** Implement solo mode for consensus ([e071b15](https://github.com/nervosnetwork/muta/commit/e071b1533b1107f65eb0f97563f011f644d73be6))
* **core/crypto:** Add secp256k1 ([8349eaa](https://github.com/nervosnetwork/muta/commit/8349eaa2817ee8c27e9e8367c89f3469e52b6f8a))
* **core/crypto:** Modify the return type to result. ([9f2424c](https://github.com/nervosnetwork/muta/commit/9f2424ca11fa300f7269f7a32195ec8bbde096e0))
* **core/network:** Support broadcast message ([#185](https://github.com/nervosnetwork/muta/issues/185)) ([992c55f](https://github.com/nervosnetwork/muta/commit/992c55f87458a38629944fb78ee69982d8329b2b))
* **core/types:** Add hash function for the header and receipts ([c982a52](https://github.com/nervosnetwork/muta/commit/c982a52ce29da7f0e783b2a7a52f1d541c15ea10))
* **executor:** Add flush for trie db. ([#240](https://github.com/nervosnetwork/muta/issues/240)) ([23fd538](https://github.com/nervosnetwork/muta/commit/23fd53849ac626cdeaabb165c0534bb90651aa90))
* **jsonrpc:** Implement filter APIs ([#190](https://github.com/nervosnetwork/muta/issues/190)) ([c97ed22](https://github.com/nervosnetwork/muta/commit/c97ed2273b6ddb2385d6d0285f2d5b4d267b130b))
* **tx-pool:** Batch broadcast transactions. ([#234](https://github.com/nervosnetwork/muta/issues/234)) ([d297b1a](https://github.com/nervosnetwork/muta/commit/d297b1a4d655fdfac25f7f5630253f7e8f6f70ea))
* add synchronizer ([#167](https://github.com/nervosnetwork/muta/issues/167)) ([38db7aa](https://github.com/nervosnetwork/muta/commit/38db7aa3f83e4a35417440e4787c5249b9eace63))
* Implement many JSONRPC APIs ([#166](https://github.com/nervosnetwork/muta/issues/166)) ([807b6a7](https://github.com/nervosnetwork/muta/commit/807b6a73cb098087179d9b086fa0070b6ced74d0))
* Implement RPC getTransactionCount ([#169](https://github.com/nervosnetwork/muta/issues/169)) ([dbf0c51](https://github.com/nervosnetwork/muta/commit/dbf0c51a17f3e285e1146eee3b5e9def08d16d50))
* rewrite network component ([#230](https://github.com/nervosnetwork/muta/issues/230)) ([585dabb](https://github.com/nervosnetwork/muta/commit/585dabb2d52dd70de7ebc26eee59345596301c1a))
* **components/jsonrpc:** Implements sendRawTransaction ([#159](https://github.com/nervosnetwork/muta/issues/159)) ([112d345](https://github.com/nervosnetwork/muta/commit/112d34582c00bea3c05d1663cf07d79aefbfa6a9))
* **core-context:** add `CommonValue` trait and `p2p_session_id` method ([#165](https://github.com/nervosnetwork/muta/issues/165)) ([216b743](https://github.com/nervosnetwork/muta/commit/216b74381c00b15ba61444cf462528ee170fcc41))
* **core/consensus:** Implements BFT ([#158](https://github.com/nervosnetwork/muta/issues/158)) ([e7a3bfd](https://github.com/nervosnetwork/muta/commit/e7a3bfd2f667c9bb8d6b9deb29a57c837ae296b9))
* **core/notify:** add notify as message-bus between components ([b53c50d](https://github.com/nervosnetwork/muta/commit/b53c50dc04090b6b0d5b6725b5c32697446aa5f8))
* **core/serialization:** Add proto file ([0bf7c59](https://github.com/nervosnetwork/muta/commit/0bf7c59200ad4a4cc7994efecaec5d8c683f175a))
* **core/storage:** Add the storage trait ([ffc8776](https://github.com/nervosnetwork/muta/commit/ffc8776b02bc0a4cf785c7c5c47a88266f186b49))
* **core/types:** Add the transactions hash calculation function. ([67d8170](https://github.com/nervosnetwork/muta/commit/67d817072c4c03b2fc2eaae5d1dc99d2d41240e0))
* **core/types:** Define serialization and deserialization methods ([f28c63d](https://github.com/nervosnetwork/muta/commit/f28c63d2b4c7b77dbe24e2b50e70cf649a6c714c))
* **database:** Add memory db ([d21a5a2](https://github.com/nervosnetwork/muta/commit/d21a5a29bd20e02f3ddd29f77c3df2963f8f3b4b))
* **jsonrpc:** support batch ([0a0c680](https://github.com/nervosnetwork/muta/commit/0a0c680993ff9be231f1ae8e583171e1f304f79b))
* **main:** add init command for genesis ([#96](https://github.com/nervosnetwork/muta/issues/96)) ([ec752b0](https://github.com/nervosnetwork/muta/commit/ec752b0602800055990fbfcc54bd2c2ab0b2cb60))
* **p2p:** Update to tentacle0.2.0-alpha.5 ([#177](https://github.com/nervosnetwork/muta/issues/177)) ([f6f83b6](https://github.com/nervosnetwork/muta/commit/f6f83b6b263579d66160cfab29b83bd5a709eeb4))
* **pubsub:** Implement pubsub components ([#143](https://github.com/nervosnetwork/muta/issues/143)) ([a079770](https://github.com/nervosnetwork/muta/commit/a079770b0e66e22552bd8cf504a9e1ba0c520d0e))
* **runtime:** add `Context` struct ([#155](https://github.com/nervosnetwork/muta/issues/155)) ([27e5aa7](https://github.com/nervosnetwork/muta/commit/27e5aa7f01f3559d2a9dd17346595c9161a9c0f6))
* Add project framework ([#24](https://github.com/nervosnetwork/muta/issues/24)) ([733ee8e](https://github.com/nervosnetwork/muta/commit/733ee8e6be7649c9aa2d772bb1dc661bd0879917))
* Add transaction pool component. ([360c935](https://github.com/nervosnetwork/muta/commit/360c93540ea77dc51551a3739e17682600d2b1b7))
* Fill main.rs ([#102](https://github.com/nervosnetwork/muta/issues/102)) ([b5b4c72](https://github.com/nervosnetwork/muta/commit/b5b4c7233efcd1c35e92248b7726ca20644800e9))
* impl cita-jsonrpc ([49e2a2d](https://github.com/nervosnetwork/muta/commit/49e2a2d22d094b2b6a2f71bc5201ccfe28308797))
* update db interface and storage interface ([#137](https://github.com/nervosnetwork/muta/issues/137)) ([36b3d07](https://github.com/nervosnetwork/muta/commit/36b3d07f23e2c7ada870cb699bf138cdd66c2860))


### Reverts

* Revert "chore: Update bft-rs (#203)" (#204) ([cc15ba9](https://github.com/nervosnetwork/muta/commit/cc15ba9ed302ab1389838a4a6c745675106179e9)), closes [#203](https://github.com/nervosnetwork/muta/issues/203) [#204](https://github.com/nervosnetwork/muta/issues/204)



# [](https://github.com/nervosnetwork/muta/compare/v0.2.0-alpha.1...v) (2020-08-03)


### Bug Fixes

* **consensus:** return an error when committing an outdated block ([#371](https://github.com/nervosnetwork/muta/issues/371)) ([b3d518b](https://github.com/nervosnetwork/muta/commit/b3d518b52658b40746ef708fa8cde5c96a39a539))
* **mempool:** Ensure that there are no duplicate transactions in the order transaction ([#379](https://github.com/nervosnetwork/muta/issues/379)) ([97708ac](https://github.com/nervosnetwork/muta/commit/97708ac385be2243344d700a0d7c928f18fd51b3))
* **storage:** test batch receipts get panic ([#373](https://github.com/nervosnetwork/muta/issues/373)) ([300a3c6](https://github.com/nervosnetwork/muta/commit/300a3c65cf0399c2ba37a3bd655e06719b660330))


### Features

* **network:** tag consensus peer ([#364](https://github.com/nervosnetwork/muta/issues/364)) ([9b27df1](https://github.com/nervosnetwork/muta/commit/9b27df1015a25792cc210c5aa0dd473a45ae885d)), closes [#354](https://github.com/nervosnetwork/muta/issues/354) [#2](https://github.com/nervosnetwork/muta/issues/2) [#3](https://github.com/nervosnetwork/muta/issues/3) [#4](https://github.com/nervosnetwork/muta/issues/4) [#5](https://github.com/nervosnetwork/muta/issues/5) [#6](https://github.com/nervosnetwork/muta/issues/6) [#7](https://github.com/nervosnetwork/muta/issues/7)
* Add global panic hook ([#376](https://github.com/nervosnetwork/muta/issues/376)) ([7382279](https://github.com/nervosnetwork/muta/commit/738227962771a6a66b85f2fd199df2e699b43adc))


### Performance Improvements

* **executor:** use inner call instead of service dispatcher ([#365](https://github.com/nervosnetwork/muta/issues/365)) ([7b1d2a3](https://github.com/nervosnetwork/muta/commit/7b1d2a32d5c20306af3868e5265bd2530dd9493b))


### BREAKING CHANGES

* **network:** - replace Validator address bytes with pubkey bytes

* change(consensus): log validator address instead of its public key

Block proposer is address instead public key

* fix: compilation failed
* **network:** - change users_cast to multicast, take peer_ids bytes instead of Address
- network bootstrap configuration now takes peer id instead of pubkey hex

* refactor(network): PeerId api



# [0.2.0-alpha.1](https://github.com/nervosnetwork/muta/compare/v0.1.2-beta...v0.2.0-alpha.1) (2020-07-22)


### Bug Fixes

* **executor:** The logic to deal with tx_hook and tx_body ([#367](https://github.com/nervosnetwork/muta/issues/367)) ([749d558](https://github.com/nervosnetwork/muta/commit/749d558b8b58a1943bfa2842dcedcc45218c0f78))
* **executor:** tx events aren't cleared on execution error ([#313](https://github.com/nervosnetwork/muta/issues/313)) ([1605cf5](https://github.com/nervosnetwork/muta/commit/1605cf59b558b97889bb431da7f81fd424b90a89))
* **proof:** Verify aggregated signature in checking proof ([#308](https://github.com/nervosnetwork/muta/issues/308)) ([d2a98b0](https://github.com/nervosnetwork/muta/commit/d2a98b06e44449ca756f135c1b235ff0d80eaf67))
* **trust_metric_test:** unreliable full node exit check ([#327](https://github.com/nervosnetwork/muta/issues/327)) ([a4ab4a6](https://github.com/nervosnetwork/muta/commit/a4ab4a6209e0978148983e88447ac2d9178fa42a))
* **WAL:** Ignore path already exist ([#304](https://github.com/nervosnetwork/muta/issues/304)) ([02df937](https://github.com/nervosnetwork/muta/commit/02df937fb6449c9b3b0b50e790e0ecf6bfc1ee3d))


### Performance Improvements

* **mempool:** parallel verifying signatures in mempool ([#359](https://github.com/nervosnetwork/muta/issues/359)) ([2ccdf1a](https://github.com/nervosnetwork/muta/commit/2ccdf1a67a40cd483749a98a1a68c37bcf1d473c))


### Reverts

* Revert "refactor(consensus)!: replace Validator address bytes with pubkey bytes (#354)" (#361) ([4dabfa2](https://github.com/nervosnetwork/muta/commit/4dabfa231961d1ec8be1ba42bf05781f55395aed)), closes [#354](https://github.com/nervosnetwork/muta/issues/354) [#361](https://github.com/nervosnetwork/muta/issues/361)


* refactor(consensus)!: replace Validator address bytes with pubkey bytes (#354) ([e4433d7](https://github.com/nervosnetwork/muta/commit/e4433d793e8a63788ec682880afc93474e0d2414)), closes [#354](https://github.com/nervosnetwork/muta/issues/354)


### Features

* **executor:** allow cancel execution units through context ([#317](https://github.com/nervosnetwork/muta/issues/317)) ([eafb489](https://github.com/nervosnetwork/muta/commit/eafb489f78f7521487c6b2d25dd9912e43f76500))
* **executor:** indenpendent tx hook states commit ([#316](https://github.com/nervosnetwork/muta/issues/316)) ([fde6450](https://github.com/nervosnetwork/muta/commit/fde645010363a4664033370e4109e4d1f08b13bc))
* **protocol:** Remove the logs bloom from block header ([#312](https://github.com/nervosnetwork/muta/issues/312)) ([ff1e0df](https://github.com/nervosnetwork/muta/commit/ff1e0df1e8a65cc480825a49eed9495cc31ecee0))
