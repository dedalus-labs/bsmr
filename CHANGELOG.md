# Changelog

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

<!-- ===----------------------------------------------------------------------=== -->
<!-- Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

## Changelog

Notable changes to Bessemer are recorded here. Release entries are generated
from conventional commits and reviewed before publication.
