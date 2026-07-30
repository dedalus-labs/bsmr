load("@bsmr_build//rules:native_rules.bzl", "alias")
load(":defs.bzl", "bsmr_bundle", "pagable_transition_alias")

oncall("build_infra")

# Need a custom transition here so that bsmr is always built with pagable enabled,
# even if its parent does not have pagable enabled.
pagable_transition_alias(
    name = "bsmr",
    actual = "//bsmr/app/bsmr:bsmr-bin",
)

bsmr_bundle(
    name = "bsmr_bundle",
    bsmr = "//bsmr:bsmr",
    bsmr_client = "//bsmr/app/bsmr:bsmr_client-bin",
    bsmr_health_check = "//bsmr/bsmr_health_check_cli:bsmr_health_check_cli",
    tpx = "//bsmr/bsmr_tpx_cli:bsmr_tpx_cli",
    visibility = ["PUBLIC"],
)

# For backcompat with bash aliases and so forth
# You can use this target to test custom builds of bsmr.
#
# Step 1: `bsmr build @fbcode//mode/opt fbcode//bsmr:symlinked_bsmr_and_tpx --out ~/bsmr`
# Step 2: Use the bsmr binary from `~/bsmr/bsmr`
#
# If you're testing on macOS, use `@fbcode//mode/opt-mac-arm64`
alias(
    name = "symlinked_bsmr_and_tpx",
    actual = ":bsmr_bundle",
)
