# Android rules

This directory contains Android rules inherited from Buck2. It is not a
supported Bessemer API: the current rules still assume tooling and targets that
are unavailable outside Meta.

Bessemer-owned Android support will live in `rules/android/`. Moving code there
requires a public SDK toolchain plus a clean example application that builds
and tests in CI. Until those contracts exist, changes here should remove private
dependencies rather than add compatibility aliases.
