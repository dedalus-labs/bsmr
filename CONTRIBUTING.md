# Contributing

`origin` is the Dedalus repository. `upstream` is the Buck2 repository and is
fetch-only.

Configure a new clone once:

```sh
git remote add upstream https://github.com/facebook/buck2.git
git remote set-url --push upstream DISABLED
git fetch upstream --tags --prune
```

All changes target `main` through pull requests. Start each branch from the
current Dedalus main branch:

```sh
git fetch origin
git switch main
git pull --ff-only origin main
git switch -c feat/eng-123-short-description
```

## Upstream syncs

Sync an explicit Buck2 release tag. Do not merge a moving `upstream/main`
without recording the exact commit under review.

```sh
upstream_release=2026-07-15
git fetch upstream "refs/tags/${upstream_release}:refs/tags/${upstream_release}"
git switch -c chore/eng-123-sync-buck2 origin/main
git merge --no-ff "${upstream_release}"
```

An upstream-sync pull request must record:

- the previous and proposed upstream commits;
- the Dedalus patches carried across the sync;
- merge conflicts and their resolutions;
- upstream, Dedalus, and performance tests run.

Keep `upstream` push-disabled. Cite exact Bazel sources when adopting Bazel
designs; a permanent Bazel remote is unnecessary because its history is not
part of this repository's ancestry.

Contributions remain licensed under [MIT](LICENSE-MIT) and
[Apache-2.0](LICENSE-APACHE).
