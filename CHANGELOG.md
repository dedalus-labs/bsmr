<!-- ===----------------------------------------------------------------------=== -->
<!-- Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

# Changelog

Notable changes to Bessemer are recorded here. Release entries are generated
from conventional commits and reviewed before publication.

## [0.0.8](https://github.com/dedalus-labs/bsmr/compare/v0.0.7...v0.0.8) (2026-09-26)


### Features

* **actions:** preserve source links in copied trees ([#300](https://github.com/dedalus-labs/bsmr/issues/300)) ([3ee9900](https://github.com/dedalus-labs/bsmr/commit/3ee99001ae2e7788108c69d607bcdbe5b113e7c7))
* **cargo:** export build-script execution settings ([#273](https://github.com/dedalus-labs/bsmr/issues/273)) ([ab18a0a](https://github.com/dedalus-labs/bsmr/commit/ab18a0ae95955c251d45ec33803aac702bcd40a2))
* **ci:** bind runner dispatches to approved revisions ([#235](https://github.com/dedalus-labs/bsmr/issues/235)) ([ee4362b](https://github.com/dedalus-labs/bsmr/commit/ee4362b218af88417f9ad2b9bc78eb92e8a98b7c))
* **ci:** build main revisions on office capacity ([#252](https://github.com/dedalus-labs/bsmr/issues/252)) ([5e95c69](https://github.com/dedalus-labs/bsmr/commit/5e95c6960ea1c99ff6eaa5562ad9c78269e67448))
* **ci:** preserve runner ownership across handoff ([#234](https://github.com/dedalus-labs/bsmr/issues/234)) ([27c3e62](https://github.com/dedalus-labs/bsmr/commit/27c3e6238e0abe611e4b5594ed9cde422a48ea14))
* **ci:** route approved native Mac builds ([#236](https://github.com/dedalus-labs/bsmr/issues/236)) ([b14dd1b](https://github.com/dedalus-labs/bsmr/commit/b14dd1b019d03cd1193dc51ed7bd59f4093a8ef1))
* **rust:** admit the pinned Rust 1.98 toolchain ([#228](https://github.com/dedalus-labs/bsmr/issues/228)) ([20c4da4](https://github.com/dedalus-labs/bsmr/commit/20c4da4234d0928091ffffa28a95d1b77c0cc823))
* **rust:** carry Cargo build-script metadata ([#283](https://github.com/dedalus-labs/bsmr/issues/283)) ([dc55008](https://github.com/dedalus-labs/bsmr/commit/dc55008d40ec7f44e2eb117574b88610706b4fb9))
* **rust:** compile locked external packages ([#226](https://github.com/dedalus-labs/bsmr/issues/226)) ([d0423cd](https://github.com/dedalus-labs/bsmr/commit/d0423cd8a6640c64540ceecb0469568fa7d0fb0a))
* **rust:** compile macros within declared inputs ([#270](https://github.com/dedalus-labs/bsmr/issues/270)) ([bb3c528](https://github.com/dedalus-labs/bsmr/commit/bb3c5286308b7e8abe35f1829e869730a2dbcddc))
* **rust:** declare extra workspace sources ([#316](https://github.com/dedalus-labs/bsmr/issues/316)) ([38fd55f](https://github.com/dedalus-labs/bsmr/commit/38fd55f35e57ce390545304d445e946eb8e37653))
* **rust:** discover Cargo workspace roots ([#329](https://github.com/dedalus-labs/bsmr/issues/329)) ([d9ac426](https://github.com/dedalus-labs/bsmr/commit/d9ac426aeaf19f549853f9c3cf81de8a3978a7aa))
* **rust:** execute Cargo integration tests ([#290](https://github.com/dedalus-labs/bsmr/issues/290)) ([fa9299b](https://github.com/dedalus-labs/bsmr/commit/fa9299b261af1e955118e27a25ad100c40406f1a))
* **rust:** execute declared Cargo build scripts ([#274](https://github.com/dedalus-labs/bsmr/issues/274)) ([728fd00](https://github.com/dedalus-labs/bsmr/commit/728fd00a6fab102fd58f30593a7245a9629e6476))
* **rust:** honor configured linker drivers ([#294](https://github.com/dedalus-labs/bsmr/issues/294)) ([e8b89c2](https://github.com/dedalus-labs/bsmr/commit/e8b89c26a6796529545e914e53dc4e49d50f5497))
* **rust:** plan package roots together ([#322](https://github.com/dedalus-labs/bsmr/issues/322)) ([5ffcc6a](https://github.com/dedalus-labs/bsmr/commit/5ffcc6a2bc8f5530169c1ff7311a42b892744413))
* **rust:** preserve declared library formats ([#276](https://github.com/dedalus-labs/bsmr/issues/276)) ([ee37902](https://github.com/dedalus-labs/bsmr/commit/ee37902a207cb64cdb2202eb5fa80da77e206c4e))
* **rust:** preserve package-qualified root selections ([#323](https://github.com/dedalus-labs/bsmr/issues/323)) ([014d060](https://github.com/dedalus-labs/bsmr/commit/014d060d779d4a823eed2abad0591d63f703d6fb))
* **rust:** run scripts from captured checkouts ([#304](https://github.com/dedalus-labs/bsmr/issues/304)) ([3b35082](https://github.com/dedalus-labs/bsmr/commit/3b350828e89c4734bbf66c2369ab9f0bc154489f))
* **rust:** support native cross-crate lto ([#232](https://github.com/dedalus-labs/bsmr/issues/232)) ([69ee145](https://github.com/dedalus-labs/bsmr/commit/69ee14563d0062d1b46dbc64c128fd6cc7f9cac8))
* **rust:** track Cargo build selections ([#231](https://github.com/dedalus-labs/bsmr/issues/231)) ([776d86d](https://github.com/dedalus-labs/bsmr/commit/776d86d3d8d338d8e065619b6540ad2cd0625ef8))
* **rust:** track checkout metadata per command ([#302](https://github.com/dedalus-labs/bsmr/issues/302)) ([4401e0a](https://github.com/dedalus-labs/bsmr/commit/4401e0af664736bed57cdf9f8f1f570069656625))
* **sandbox:** activate declared namespace execution ([#267](https://github.com/dedalus-labs/bsmr/issues/267)) ([116a6f2](https://github.com/dedalus-labs/bsmr/commit/116a6f2a4cc48e9d5231bd9b89e2af2c51e16476))
* **sandbox:** share immutable runtime file payloads ([#308](https://github.com/dedalus-labs/bsmr/issues/308)) ([c4bb1cf](https://github.com/dedalus-labs/bsmr/commit/c4bb1cf4e33b505954b04974707ef1884f8bd345))
* **sandbox:** validate local output trees ([#265](https://github.com/dedalus-labs/bsmr/issues/265)) ([aeda83c](https://github.com/dedalus-labs/bsmr/commit/aeda83ce09d69118f09b9684bf08cda30cc2de96))
* **sandbox:** verify namespace runtime snapshots ([#264](https://github.com/dedalus-labs/bsmr/issues/264)) ([eedc81d](https://github.com/dedalus-labs/bsmr/commit/eedc81def9efb658e24b22f308b08537b12e9fcd))


### Bug Fixes

* **archive:** extract sources with the current owner ([#277](https://github.com/dedalus-labs/bsmr/issues/277)) ([cdfec25](https://github.com/dedalus-labs/bsmr/commit/cdfec25183e92b5759377f3a59261498981f05ba))
* **cargo:** preserve dependency lint caps ([#278](https://github.com/dedalus-labs/bsmr/issues/278)) ([ee264d6](https://github.com/dedalus-labs/bsmr/commit/ee264d6132d453b815abe0813b42a653c81779c1))
* **ci:** preserve Cargo between compiler cache saves ([#258](https://github.com/dedalus-labs/bsmr/issues/258)) ([7816057](https://github.com/dedalus-labs/bsmr/commit/78160579ba95bf874474350e8d966a47041c07ee))
* **ci:** retain the Cargo planner dependency cache ([#238](https://github.com/dedalus-labs/bsmr/issues/238)) ([ff13ce4](https://github.com/dedalus-labs/bsmr/commit/ff13ce41b4a48b2decec833220733f5e2550bc97))
* **ci:** reuse the qualified engine build profile ([#280](https://github.com/dedalus-labs/bsmr/issues/280)) ([5f5d0fd](https://github.com/dedalus-labs/bsmr/commit/5f5d0fdeb3c22cb602aadcd05a945d1068354655))
* **ci:** wait for owned runner observations ([#256](https://github.com/dedalus-labs/bsmr/issues/256)) ([fba5b1a](https://github.com/dedalus-labs/bsmr/commit/fba5b1abc11cb73f02cfa5fe4d86e8ee911db28f))
* **cxx:** name macOS shared libraries correctly ([#330](https://github.com/dedalus-labs/bsmr/issues/330)) ([cd63347](https://github.com/dedalus-labs/bsmr/commit/cd633475b2dc75557e02402dd3d17a0353c9c242))
* **git:** isolate pinned source acquisition ([#225](https://github.com/dedalus-labs/bsmr/issues/225)) ([56aae4a](https://github.com/dedalus-labs/bsmr/commit/56aae4aa55fa4d6563555757b4a7710787e38c21))
* **rust:** bind compiler manifest paths to package sources ([#309](https://github.com/dedalus-labs/bsmr/issues/309)) ([c8e2ab0](https://github.com/dedalus-labs/bsmr/commit/c8e2ab018e93bf178653e684a2f378ae640a95af))
* **rust:** build Cargo directory defaults ([#328](https://github.com/dedalus-labs/bsmr/issues/328)) ([c0f190a](https://github.com/dedalus-labs/bsmr/commit/c0f190a82cdc7f536a8eef4d5e36914e61865618))
* **rust:** declare cargo cfg names for lint checking ([#305](https://github.com/dedalus-labs/bsmr/issues/305)) ([b0de8c4](https://github.com/dedalus-labs/bsmr/commit/b0de8c41869cac88f1afdc507db7c492dd6a3fcc))
* **rust:** forward build-script linker arguments ([#285](https://github.com/dedalus-labs/bsmr/issues/285)) ([36c12a2](https://github.com/dedalus-labs/bsmr/commit/36c12a249ca650fc91b9c0a372a608ae51940292))
* **rust:** honor build-script diagnostics ([#223](https://github.com/dedalus-labs/bsmr/issues/223)) ([3cc3db7](https://github.com/dedalus-labs/bsmr/commit/3cc3db7b08633dad217375eb89d519828a6c51e1))
* **rust:** honor native compiler toolchains ([#279](https://github.com/dedalus-labs/bsmr/issues/279)) ([edc0dd5](https://github.com/dedalus-labs/bsmr/commit/edc0dd5a3ca02f0413d386877fce527fca2379b2))
* **rust:** materialize transitive dependency inputs ([#282](https://github.com/dedalus-labs/bsmr/issues/282)) ([3d5657f](https://github.com/dedalus-labs/bsmr/commit/3d5657f9d827ebc9fe2de631adf36d22ebd174a9))
* **rust:** pass manifest links to build scripts ([#307](https://github.com/dedalus-labs/bsmr/issues/307)) ([5956108](https://github.com/dedalus-labs/bsmr/commit/59561084779e1b77590764de52088af04caefdc5))
* **rust:** pin compiler archive lengths ([#286](https://github.com/dedalus-labs/bsmr/issues/286)) ([5be9590](https://github.com/dedalus-labs/bsmr/commit/5be9590e97f31c4e35fe0b61bfce5d32e01bb197))
* **rust:** preserve build-script runtime contracts ([#272](https://github.com/dedalus-labs/bsmr/issues/272)) ([d2d3322](https://github.com/dedalus-labs/bsmr/commit/d2d3322e2c2712534e1806d08ee1caf23b2bcfd9))
* **rust:** preserve declared compiler inputs ([#314](https://github.com/dedalus-labs/bsmr/issues/314)) ([7927cc7](https://github.com/dedalus-labs/bsmr/commit/7927cc716f7449a730c1a50d43fc39268f42c33e))
* **rust:** preserve optional Cargo target selection ([#326](https://github.com/dedalus-labs/bsmr/issues/326)) ([3d18f7d](https://github.com/dedalus-labs/bsmr/commit/3d18f7da269847d3cae43d69c992034a4f73d410))
* **rust:** preserve package source layouts ([#310](https://github.com/dedalus-labs/bsmr/issues/310)) ([26c939f](https://github.com/dedalus-labs/bsmr/commit/26c939f7380f4fb709252d34f2508161b79fc9ad))
* **rust:** preserve test source locations ([#292](https://github.com/dedalus-labs/bsmr/issues/292)) ([723590d](https://github.com/dedalus-labs/bsmr/commit/723590d48adf135e506269426a5d2dd77f7ae9dc))
* **rust:** retain complete git dependency trees ([#306](https://github.com/dedalus-labs/bsmr/issues/306)) ([6d73870](https://github.com/dedalus-labs/bsmr/commit/6d73870cb4e7617098740c6c74a83285bfceb892))
* **rust:** retain declared target entrypoint paths ([#327](https://github.com/dedalus-labs/bsmr/issues/327)) ([beb9b86](https://github.com/dedalus-labs/bsmr/commit/beb9b863865ff8502719183b242b7b77c22a6dbf))
* **rust:** retain excluded path dependency sources ([#230](https://github.com/dedalus-labs/bsmr/issues/230)) ([3ba50df](https://github.com/dedalus-labs/bsmr/commit/3ba50df0e46389d96429202cecc46b0213a3850f))
* **rust:** retain workspace test fixtures ([#293](https://github.com/dedalus-labs/bsmr/issues/293)) ([6650679](https://github.com/dedalus-labs/bsmr/commit/66506795b28d6ae87e4a80b1b5fece89706756fd))
* **rust:** reuse locked git source archives ([#298](https://github.com/dedalus-labs/bsmr/issues/298)) ([13b1f49](https://github.com/dedalus-labs/bsmr/commit/13b1f496a67ccbd43d077d9d454c1e5a8df599a5))
* **rust:** share joint compilation selections ([#324](https://github.com/dedalus-labs/bsmr/issues/324)) ([b2caaf2](https://github.com/dedalus-labs/bsmr/commit/b2caaf29b32fed14441144af2e71a0a5b24f646f))
* **rust:** validate native filegroup inputs ([#312](https://github.com/dedalus-labs/bsmr/issues/312)) ([d6e1749](https://github.com/dedalus-labs/bsmr/commit/d6e174992b80f74aea52839238c6558bc6d0ef68))
* **rust:** validate selected compiler flags ([#229](https://github.com/dedalus-labs/bsmr/issues/229)) ([143a50a](https://github.com/dedalus-labs/bsmr/commit/143a50a4879a77f14387e126190ad2970be93f1a))
* **sandbox:** admit complete compiler runtime archives ([#281](https://github.com/dedalus-labs/bsmr/issues/281)) ([eab05bd](https://github.com/dedalus-labs/bsmr/commit/eab05bd1092001a73ab66caf59a1cdb1c4067097))
* **sandbox:** bind isolation before action reuse ([#266](https://github.com/dedalus-labs/bsmr/issues/266)) ([34efa71](https://github.com/dedalus-labs/bsmr/commit/34efa71822792b42c45afecc69160597136ad01f))
* **sandbox:** stage native inputs without VM transport ([#288](https://github.com/dedalus-labs/bsmr/issues/288)) ([13818b4](https://github.com/dedalus-labs/bsmr/commit/13818b43c073ca5bc2a16297d6dccfa398a936a1))
* **sandbox:** stream large native input trees ([#311](https://github.com/dedalus-labs/bsmr/issues/311)) ([d184ebb](https://github.com/dedalus-labs/bsmr/commit/d184ebb8639c28421cb98c612a0e8fe05a84b7ef))
* **source:** preserve literal unix symlink targets ([#299](https://github.com/dedalus-labs/bsmr/issues/299)) ([aee44da](https://github.com/dedalus-labs/bsmr/commit/aee44dacdce9000c28849b0a4b424129b7c6283f))
* **test:** honor sandbox environment isolation ([#289](https://github.com/dedalus-labs/bsmr/issues/289)) ([0f36c5f](https://github.com/dedalus-labs/bsmr/commit/0f36c5f5a6926204a6323225b73e3313214f5b8b))
* **watcher:** invalidate state on pathless overflow ([#320](https://github.com/dedalus-labs/bsmr/issues/320)) ([2cc17c9](https://github.com/dedalus-labs/bsmr/commit/2cc17c9635a861e10f7c9dd443c17f66bcd856e8))


### Performance Improvements

* **sandbox:** retain verified runtime snapshots ([#321](https://github.com/dedalus-labs/bsmr/issues/321)) ([15f18b6](https://github.com/dedalus-labs/bsmr/commit/15f18b68e5d95b7a159ccdf0ee22fec98b1d86a6))

## [0.0.7](https://github.com/dedalus-labs/bsmr/compare/v0.0.6...v0.0.7) (2026-09-21)


### Features

* **rust:** describe cargo entrypoints ([#212](https://github.com/dedalus-labs/bsmr/issues/212)) ([a064213](https://github.com/dedalus-labs/bsmr/commit/a064213865287448d3d9d8573b1b19564a73c783))
* **rust:** execute configured cargo plans ([#214](https://github.com/dedalus-labs/bsmr/issues/214)) ([55b1eac](https://github.com/dedalus-labs/bsmr/commit/55b1eacf9083847fc7c08c2702640be52700ad93))
* **rust:** export native compiler metadata ([#207](https://github.com/dedalus-labs/bsmr/issues/207)) ([043418a](https://github.com/dedalus-labs/bsmr/commit/043418a1b3a9f35167d69a250658fc032b746794))
* **rust:** lower configured cargo units ([#211](https://github.com/dedalus-labs/bsmr/issues/211)) ([5a4bf46](https://github.com/dedalus-labs/bsmr/commit/5a4bf460157fd40396692d6ba88b8e265e75ac3b))
* **rust:** plan configured cargo units ([#204](https://github.com/dedalus-labs/bsmr/issues/204)) ([f0d77c3](https://github.com/dedalus-labs/bsmr/commit/f0d77c32bdce305397cb9db9e40108c74804245a))
* **rust:** preserve literal compiler inputs ([#203](https://github.com/dedalus-labs/bsmr/issues/203)) ([d7f1124](https://github.com/dedalus-labs/bsmr/commit/d7f112478b8046c1e3e2b0b0fc9323c57c201809))
* **rust:** verify acquired cargo sources ([#208](https://github.com/dedalus-labs/bsmr/issues/208)) ([46080cf](https://github.com/dedalus-labs/bsmr/commit/46080cfdf0343c8159fa46bb34329a566978029a))


### Bug Fixes

* **release:** commit every product version file ([#218](https://github.com/dedalus-labs/bsmr/issues/218)) ([7edcd2c](https://github.com/dedalus-labs/bsmr/commit/7edcd2c173a23654d8eb7afb268b136cab1d70dc))
* **rust:** bound cargo compiler flags ([#205](https://github.com/dedalus-labs/bsmr/issues/205)) ([1a9abfc](https://github.com/dedalus-labs/bsmr/commit/1a9abfcb106ef7d5a2bf40ffabf2f96dcc49fcd6))
* **rust:** isolate cargo source ownership ([#206](https://github.com/dedalus-labs/bsmr/issues/206)) ([74ced4e](https://github.com/dedalus-labs/bsmr/commit/74ced4ea12eb2944c92bd7283472089d7fce0740))
* **rust:** materialize cargo library builds ([#217](https://github.com/dedalus-labs/bsmr/issues/217)) ([51e66aa](https://github.com/dedalus-labs/bsmr/commit/51e66aa212e640839c26426a2daaa2f7a5ed0762))
* **rust:** validate declared source directory inputs ([#201](https://github.com/dedalus-labs/bsmr/issues/201)) ([e719f14](https://github.com/dedalus-labs/bsmr/commit/e719f14624566799c8d28d9d19ba459918a47ccd))

## [0.0.6](https://github.com/dedalus-labs/bsmr/compare/v0.0.5...v0.0.6) (2026-09-21)


### Features

* **pnpm:** execute native scripts with checked outputs ([#194](https://github.com/dedalus-labs/bsmr/issues/194)) ([16c9b63](https://github.com/dedalus-labs/bsmr/commit/16c9b63bd39dc2bf69930d53a860f20f7631c8f6))
* **rust:** acquire pinned compiler artifacts ([#179](https://github.com/dedalus-labs/bsmr/issues/179)) ([000ca79](https://github.com/dedalus-labs/bsmr/commit/000ca79ea48c84fbeb5a5f13cc60acf11e2c5211))
* **rust:** infer native builds from cargo manifests ([#181](https://github.com/dedalus-labs/bsmr/issues/181)) ([f291f39](https://github.com/dedalus-labs/bsmr/commit/f291f3992e8e801a512085671591f666a04cdf04))
* **rust:** lower resolved graphs into native rules ([#180](https://github.com/dedalus-labs/bsmr/issues/180)) ([3b3fd1a](https://github.com/dedalus-labs/bsmr/commit/3b3fd1a0b2097295c084ceb9fdfc673c4034d626))


### Bug Fixes

* **cache:** publish finalized artifact paths ([#188](https://github.com/dedalus-labs/bsmr/issues/188)) ([3678c9f](https://github.com/dedalus-labs/bsmr/commit/3678c9fee2d6f61c86463f271f2602eb9e9d28b5))
* **ci:** stop rust aggregation when a run is canceled ([#200](https://github.com/dedalus-labs/bsmr/issues/200)) ([28d00ed](https://github.com/dedalus-labs/bsmr/commit/28d00edd1dcc36febc725c358dd172080c82fca5))
* **pnpm:** execute declared workspace binaries ([#199](https://github.com/dedalus-labs/bsmr/issues/199)) ([1b1630b](https://github.com/dedalus-labs/bsmr/commit/1b1630b240ecf54d6978fc6b57a890b0291c468d))
* **release:** complete published version metadata ([#170](https://github.com/dedalus-labs/bsmr/issues/170)) ([26dbf15](https://github.com/dedalus-labs/bsmr/commit/26dbf15cf458f42048fface2204d46d1a08a2feb))
* **rust:** validate compiler inputs before caching ([#178](https://github.com/dedalus-labs/bsmr/issues/178)) ([c591c57](https://github.com/dedalus-labs/bsmr/commit/c591c57a55fea64bc096b7474086f8fd9b2041c8))


### Performance Improvements

* **pnpm:** publish installed workspaces in place ([#174](https://github.com/dedalus-labs/bsmr/issues/174)) ([fc35d0d](https://github.com/dedalus-labs/bsmr/commit/fc35d0dd21117a1f41eb7ea558e8a87b94c2aa32))
* **rust:** stage only cargo target entrypoints ([#185](https://github.com/dedalus-labs/bsmr/issues/185)) ([1abc00a](https://github.com/dedalus-labs/bsmr/commit/1abc00a5d0404a71ae6625b0f2d81d8533858fd8))

## [0.0.5](https://github.com/dedalus-labs/bsmr/compare/v0.0.4...v0.0.5) (2026-09-15)


### Bug Fixes

* **deps:** update h2 to 0.4.19 ([#168](https://github.com/dedalus-labs/bsmr/issues/168)) ([678df2e](https://github.com/dedalus-labs/bsmr/commit/678df2e1f2271bc833b5fe276410c76aff216fd6))
* **release:** require immutable published versions ([#167](https://github.com/dedalus-labs/bsmr/issues/167)) ([302fdec](https://github.com/dedalus-labs/bsmr/commit/302fdec71866b3ee1e84b0d5b399190e89775737))

## [0.0.4](https://github.com/dedalus-labs/bsmr/compare/v0.0.3...v0.0.4) (2026-09-15)


### Features

* **go:** cache explicit internal links ([#164](https://github.com/dedalus-labs/bsmr/issues/164)) ([139ad4b](https://github.com/dedalus-labs/bsmr/commit/139ad4b74f7e83c64dbec8671d6749c2d4b4a262))
* **go:** cache pinned compiler actions ([#163](https://github.com/dedalus-labs/bsmr/issues/163)) ([533f8ae](https://github.com/dedalus-labs/bsmr/commit/533f8ae042ca582c7f452365724904365a0e71d1))


### Bug Fixes

* **cargo:** publish native builds to the local cache ([#156](https://github.com/dedalus-labs/bsmr/issues/156)) ([f1a4ae3](https://github.com/dedalus-labs/bsmr/commit/f1a4ae36b9b3f07eb3ad1176852fe20025f80b24))
* **go:** retain embedded files in internal tests ([#162](https://github.com/dedalus-labs/bsmr/issues/162)) ([248aea6](https://github.com/dedalus-labs/bsmr/commit/248aea65dd76307141856d3bff44a91987b1c260))
* **release:** advance past the historical mutable release ([#165](https://github.com/dedalus-labs/bsmr/issues/165)) ([363f228](https://github.com/dedalus-labs/bsmr/commit/363f22858b102f6aa4ee690433c920f9b46d059d))


### Performance Improvements

* **cache:** deduplicate concurrent action misses ([#158](https://github.com/dedalus-labs/bsmr/issues/158)) ([9ea30a8](https://github.com/dedalus-labs/bsmr/commit/9ea30a8a103c60426d5c8147c4b956f23a5aa888))
* **cache:** restore outputs with filesystem clones ([#157](https://github.com/dedalus-labs/bsmr/issues/157)) ([ae0ba46](https://github.com/dedalus-labs/bsmr/commit/ae0ba461b489c4b1469c75455097698c6cc0fd50))

## [0.0.3](https://github.com/dedalus-labs/bsmr/compare/v0.0.2...v0.0.3) (2026-08-22)


### Features

* complete Bessemer identity cutover ([#138](https://github.com/dedalus-labs/bsmr/issues/138)) ([b192ce2](https://github.com/dedalus-labs/bsmr/commit/b192ce27ab027a5527f7566d18238f4e1e6226a4))
* **core:** rename output root ([#130](https://github.com/dedalus-labs/bsmr/issues/130)) ([8f6c745](https://github.com/dedalus-labs/bsmr/commit/8f6c745bb2369bd9b7809b86521f62ab7bcb232e))
* **sandbox:** isolate actions with Firecracker ([#65](https://github.com/dedalus-labs/bsmr/issues/65)) ([f0ffa0e](https://github.com/dedalus-labs/bsmr/commit/f0ffa0e273110072e82e4eaea922e0cf6a5dbc4d))


### Bug Fixes

* **release:** compile Firecracker transport on Windows ([#143](https://github.com/dedalus-labs/bsmr/issues/143)) ([8bd3060](https://github.com/dedalus-labs/bsmr/commit/8bd30603d2d88fd27545cdd3ecbd5b74f3d5d537))
* **tools:** follow output root rename ([#131](https://github.com/dedalus-labs/bsmr/issues/131)) ([1e3b3e1](https://github.com/dedalus-labs/bsmr/commit/1e3b3e1f4cc28d8cea8ebb30e2a943137d880c58))


### Performance Improvements

* **ci:** classify exact merge groups ([#79](https://github.com/dedalus-labs/bsmr/issues/79)) ([b9c2acf](https://github.com/dedalus-labs/bsmr/commit/b9c2acfd8c12e1d71659f8ed44ae1524796ef4da))

## [0.0.2](https://github.com/dedalus-labs/bsmr/compare/v0.0.1...v0.0.2) (2026-08-20)


### Features

* **node:** catalog active native runtimes ([#118](https://github.com/dedalus-labs/bsmr/issues/118)) ([0ba5a13](https://github.com/dedalus-labs/bsmr/commit/0ba5a13abf88e419ed12ebca8fd0ff78dca91c30))
* **pnpm:** honor exact workspace node runtime ([#119](https://github.com/dedalus-labs/bsmr/issues/119)) ([13414c6](https://github.com/dedalus-labs/bsmr/commit/13414c6619e183ca316dadcc73fc50629c7b0ecd))


### Bug Fixes

* **release:** harden post-release synchronization ([#123](https://github.com/dedalus-labs/bsmr/issues/123)) ([dca546b](https://github.com/dedalus-labs/bsmr/commit/dca546b6e9564360d5c60a5bf1bceea175cfec4c))
* **typescript:** preserve declared source symlinks ([#120](https://github.com/dedalus-labs/bsmr/issues/120)) ([56ed73f](https://github.com/dedalus-labs/bsmr/commit/56ed73f8309f3835c368e38d2fbe24029c5e5e1e))

## 0.0.1 (2026-08-20)


### Features

* **cli:** add a beginner help surface ([#59](https://github.com/dedalus-labs/bsmr/issues/59)) ([a148518](https://github.com/dedalus-labs/bsmr/commit/a14851842e26591e44ed9392f34d64a311f92484))
* **core:** add canonical version-set identity ([#44](https://github.com/dedalus-labs/bsmr/issues/44)) ([1d702eb](https://github.com/dedalus-labs/bsmr/commit/1d702eb047ea44d9300a6de41ef3bb237e5955e1))
* **events:** expose stable test-attempt observations ([#73](https://github.com/dedalus-labs/bsmr/issues/73)) ([1b655bc](https://github.com/dedalus-labs/bsmr/commit/1b655bc55feb1597b1561d2647444d6af5f8605a))
* **go:** add native hermetic builds ([#63](https://github.com/dedalus-labs/bsmr/issues/63)) ([1cb5638](https://github.com/dedalus-labs/bsmr/commit/1cb5638aab13dafd5d152d2fc583715ce13ad877))
* **node:** land native pnpm and TypeScript stack ([#92](https://github.com/dedalus-labs/bsmr/issues/92)) ([95c02ca](https://github.com/dedalus-labs/bsmr/commit/95c02ca767684d11322dc15c905d2f1db70aa5fc))
* **pnpm:** add frozen install adapter ([#43](https://github.com/dedalus-labs/bsmr/issues/43)) ([105e195](https://github.com/dedalus-labs/bsmr/commit/105e1951feac166a10d2f83d0f1e4a1b68bdfe65))
* **project:** unify native TypeScript and Rust builds ([#57](https://github.com/dedalus-labs/bsmr/issues/57)) ([8dc2791](https://github.com/dedalus-labs/bsmr/commit/8dc2791ee1f8fd8421b83f69bdf935414741c1fd))


### Bug Fixes

* **ci:** avoid blocked pnpm action ([#35](https://github.com/dedalus-labs/bsmr/issues/35)) ([84b17e3](https://github.com/dedalus-labs/bsmr/commit/84b17e3f022c75b2898eb82a00c2e5636f427b8a))
* **ci:** skip unaffected Rust lanes ([#27](https://github.com/dedalus-labs/bsmr/issues/27)) ([8636444](https://github.com/dedalus-labs/bsmr/commit/8636444796c884cb4bc4f4c9795838bd21ea1c0d))
* **core:** close incremental correctness races ([#49](https://github.com/dedalus-labs/bsmr/issues/49)) ([a6c3822](https://github.com/dedalus-labs/bsmr/commit/a6c3822dd0a9765ceef2ea78b44c68f39668b540))
* **deps:** make Rust updates compatibility-safe ([#106](https://github.com/dedalus-labs/bsmr/issues/106)) ([8b26a95](https://github.com/dedalus-labs/bsmr/commit/8b26a959961b5cb3d2dc12829f00f7d0da2a38d9))
* **deps:** patch lru use-after-free ([#64](https://github.com/dedalus-labs/bsmr/issues/64)) ([29721b7](https://github.com/dedalus-labs/bsmr/commit/29721b72f371460e81f90496353141b678140fde))
* **deps:** patch lru use-after-free ([#74](https://github.com/dedalus-labs/bsmr/issues/74)) ([e5b84da](https://github.com/dedalus-labs/bsmr/commit/e5b84da58091bece03ad0f2611d200205a6a5328))
* **governance:** own maintained Bessemer modules ([#11](https://github.com/dedalus-labs/bsmr/issues/11)) ([e7de592](https://github.com/dedalus-labs/bsmr/commit/e7de59216db44a90099f448b6a45f1f7baff4fa6))
* **governance:** scope Bessemer code ownership ([#10](https://github.com/dedalus-labs/bsmr/issues/10)) ([98e097e](https://github.com/dedalus-labs/bsmr/commit/98e097e28a8ca37b253d05b0e3c1cb99b2b65db6))
* **materializer:** restore missing CAS outputs ([#53](https://github.com/dedalus-labs/bsmr/issues/53)) ([9e62b5e](https://github.com/dedalus-labs/bsmr/commit/9e62b5ebaa8a6262b31e8ea6c1b8ee4f8c20c0bc))
* **native:** isolate polyglot workspace analysis ([#107](https://github.com/dedalus-labs/bsmr/issues/107)) ([adafc91](https://github.com/dedalus-labs/bsmr/commit/adafc91cb2457be15120d70c6ae4fd5651084ee8))
* **release:** finalize 0.0.1 bootstrap ([#115](https://github.com/dedalus-labs/bsmr/issues/115)) ([f9c10d5](https://github.com/dedalus-labs/bsmr/commit/f9c10d576d6de440c5c19edda7a52a870e73f154))
* **release:** pass workspace as a string ([#116](https://github.com/dedalus-labs/bsmr/issues/116)) ([18791aa](https://github.com/dedalus-labs/bsmr/commit/18791aa340db015cdb7cd6f0b3f0a1ab3a57c82a))
* **release:** repair release PR synchronization ([#114](https://github.com/dedalus-labs/bsmr/issues/114)) ([749c673](https://github.com/dedalus-labs/bsmr/commit/749c6737d402f215354bd3ce7de6c3e2b1e11fca))
* **release:** stop bundled CLI execution ([#117](https://github.com/dedalus-labs/bsmr/issues/117)) ([3793f99](https://github.com/dedalus-labs/bsmr/commit/3793f99f60915834a94c8f433b4f821d2f1c19a9))
* **release:** use repository token for cargo-dist ([#112](https://github.com/dedalus-labs/bsmr/issues/112)) ([79ceb20](https://github.com/dedalus-labs/bsmr/commit/79ceb20251f3c00238fe22037f6f09a3e2d53124))
* **typescript:** preserve package-local config imports ([#108](https://github.com/dedalus-labs/bsmr/issues/108)) ([be58413](https://github.com/dedalus-labs/bsmr/commit/be5841364bd2f426d66abdfc13296a5d04de75d3))


### Performance Improvements

* **benchmarks:** generate orchestration fixtures ([#50](https://github.com/dedalus-labs/bsmr/issues/50)) ([9b02a03](https://github.com/dedalus-labs/bsmr/commit/9b02a037049909334e40648af0682628b35e3b19))
* **benchmarks:** measure warm output restoration ([#54](https://github.com/dedalus-labs/bsmr/issues/54)) ([1945f32](https://github.com/dedalus-labs/bsmr/commit/1945f32eace7e6512a7897c87b2db1372c8d7351))
* **benchmarks:** run correctness-gated comparisons ([#51](https://github.com/dedalus-labs/bsmr/issues/51)) ([39edf6d](https://github.com/dedalus-labs/bsmr/commit/39edf6de8e39eca273a76f5dd9a05c88c01f6e66))
* **ci:** parallelize Rust checks ([#22](https://github.com/dedalus-labs/bsmr/issues/22)) ([388201c](https://github.com/dedalus-labs/bsmr/commit/388201cf91e445ee8b8ddf69b8de8889edf4e2b8))
