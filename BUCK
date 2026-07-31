load("@bsmr_build//rules:native_rules.bzl", "alias")
load(":defs.bzl", "bsmr_bundle", "pagable_transition_alias")

oncall("build_infra")

# Need a custom transition here so that bsmr is always built with pagable enabled,
# even if its parent does not have pagable enabled.
pagable_transition_alias(
    name = "bsmr",
    actual = "root//app/bsmr:bsmr-bin",
)

bsmr_bundle(
    name = "bsmr_bundle",
    bsmr = "root//:bsmr",
    bsmr_client = "root//app/bsmr:bsmr_client-bin",
    bsmr_health_check = "root//bsmr_health_check_cli:bsmr_health_check_cli",
    tpx = "root//bsmr_tpx_cli:bsmr_tpx_cli",
    visibility = ["PUBLIC"],
)

# For backcompat with bash aliases and so forth
# You can use this target to test custom builds of bsmr.
#
# Step 1: `bsmr build @upstream//mode/opt root//:symlinked_bsmr_and_tpx --out ~/bsmr`
# Step 2: Use the bsmr binary from `~/bsmr/bsmr`
#
# If you're testing on macOS, use `@upstream//mode/opt-mac-arm64`
alias(
    name = "symlinked_bsmr_and_tpx",
    actual = ":bsmr_bundle",
)
