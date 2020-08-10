# [](https://github.com/nervosnetwork/muta/compare/v0.2.0-beta.3...v) (2020-08-10)


### Bug Fixes

* load hrp before deserializing genesis payload to take hrp effect ([#405](https://github.com/nervosnetwork/muta/issues/405)) ([828e6d5](https://github.com/nervosnetwork/muta/commit/828e6d539cf4da9cf042c450418e75a944315014))

* service field of event is not binded to ctx.service ([#405](https://github.com/nervosnetwork/muta/issues/408)) ([120f1ee](https://github.com/nervosnetwork/muta/commit/120f1ee125593d7def075460f34fc3dcd5a8fe83))

### Features

* **api:** Support enabled TLS ([#402](https://github.com/nervosnetwork/muta/issues/402)) ([c2908a3](https://github.com/nervosnetwork/muta/commit/c2908a3ba6a5ab1219ddc9b14ff6d7320cf70228))


### Performance Improvements

* **state:** add state cache for trieDB ([#404](https://github.com/nervosnetwork/muta/issues/404)) ([2a08c14](https://github.com/nervosnetwork/muta/commit/2a08c147571707507b72882788fd51f7a799f3ec))



# [0.2.0-beta.3](https://github.com/nervosnetwork/muta/compare/v0.2.0-beta.2...v0.2.0-beta.3) (2020-08-07)


### Bug Fixes

* **apm:** Return the correct time ([#400](https://github.com/nervosnetwork/muta/issues/400)) ([fd6549a](https://github.com/nervosnetwork/muta/commit/fd6549a6352633cee7b5b747448129df7a0532ca))


### Features

* **network:** limit connections from same ip ([#388](https://github.com/nervosnetwork/muta/issues/388)) ([dc78c13](https://github.com/nervosnetwork/muta/commit/dc78c13b8aa25f3e4535e588149042f6345e4d25))
* **network:** limit inbound and outbound connections ([#393](https://github.com/nervosnetwork/muta/issues/393)) ([3a3111e](https://github.com/nervosnetwork/muta/commit/3a3111e1e332529bc8636c54526920c292c04f8a))
* **sync:** Limit the maximum height of once sync ([#390](https://github.com/nervosnetwork/muta/issues/390)) ([f951a95](https://github.com/nervosnetwork/muta/commit/f951a953daf307ffc98b4df0fe1a77a6a810ac71))



# [0.2.0-beta.2](https://github.com/nervosnetwork/muta/compare/v0.2.0-beta.1...v0.2.0-beta.2) (2020-08-04)


### Bug Fixes

* **consensus:** Add timestamp checking ([#377](https://github.com/nervosnetwork/muta/issues/377)) ([382ede9](https://github.com/nervosnetwork/muta/commit/382ede9367b910a06b59f3562ecd28ab8100d39e))


### Features

* **benchmark:** add a perf benchmark macro ([#391](https://github.com/nervosnetwork/muta/issues/391)) ([eb24311](https://github.com/nervosnetwork/muta/commit/eb2431149b6865a82d0e4286536f65319a5e1d1f))
* **Cargo:** add random leader feature for muta ([#385](https://github.com/nervosnetwork/muta/issues/385)) ([43da977](https://github.com/nervosnetwork/muta/commit/43da9772b22b97ab4797b80ce5161f1a49827543))


### Performance Improvements

* **metrics:** Add metrics of state ([#397](https://github.com/nervosnetwork/muta/issues/397)) ([5822764](https://github.com/nervosnetwork/muta/commit/5822764240f8b4e8cfeca4bccf7d399a0bf71897))



# [0.2.0-beta.1](https://github.com/nervosnetwork/muta/compare/v0.2.0-alpha.1...v0.2.0-beta.1) (2020-08-03)


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


### BREAKING CHANGES

* - replace Validator address bytes with pubkey bytes

* change(consensus): log validator address instead of its public key

Block proposer is address instead public key

* fix: compilation failed

