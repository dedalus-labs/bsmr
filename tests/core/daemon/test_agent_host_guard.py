# ===----------------------------------------------------------------------===
# Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc
# Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# pyre-strict


import re

from bsmr.tests.e2e_util.api.buck import Buck
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.buck_workspace import buck_test
from bsmr.tests.e2e_util.helper.golden import golden, sanitize_stderr

# Cgroup of an agent-originated daemon; `BSMR_TEST_DAEMON_ORIGINATING_CGROUP`
# overrides the real cgroup, which is unavailable since tests disable the cgroup spawner.
_AGENT_CGROUP = "/user.slice/3pai_sandbox.slice/bsmr.scope"
_NON_AGENT_CGROUP = "/user.slice/user-1000.slice/bsmr.scope"


def _write_bsmrconfig_local(buck: Buck, contents: str) -> None:
    with open(buck.cwd / ".bsmrconfig.local", "w") as f:
        f.write(contents)


def _sanitize_denial(stderr: str, project_root: str) -> str:
    # Replace the project root path before `sanitize_stderr`, which would
    # otherwise rewrite the hash inside the path and break a literal match.
    s = stderr.replace(project_root, "<PROJECT_ROOT>")
    # The hostname is host-specific; normalize it so the golden is portable.
    s = re.sub(r"host `[^`]+`", "host `<HOSTNAME>`", s)
    return sanitize_stderr(s)


@buck_test()
async def test_agent_host_guard_disabled(buck: Buck) -> None:
    # No glob configured -> feature off, build succeeds even from an agent cgroup.
    await buck.build(
        ":pass",
        env={"BSMR_TEST_DAEMON_ORIGINATING_CGROUP": _AGENT_CGROUP},
    )


@buck_test()
async def test_agent_host_guard_denied(buck: Buck) -> None:
    _write_bsmrconfig_local(
        buck,
        "[bsmr]\nagent_hostname_fail_v2_glob=*\n",
    )
    result = await expect_failure(
        buck.build(
            ":pass",
            env={"BSMR_TEST_DAEMON_ORIGINATING_CGROUP": _AGENT_CGROUP},
        ),
    )
    golden(
        output=_sanitize_denial(result.stderr, str(buck.cwd)),
        rel_path="golden/denied.golden.stderr",
    )


@buck_test()
async def test_agent_host_guard_denied_with_context(buck: Buck) -> None:
    _write_bsmrconfig_local(
        buck,
        "[bsmr]\nagent_hostname_fail_v2_glob=*\nagent_hostname_fail_v2_context=See S123456\n",
    )
    result = await expect_failure(
        buck.build(
            ":pass",
            env={"BSMR_TEST_DAEMON_ORIGINATING_CGROUP": _AGENT_CGROUP},
        ),
    )
    golden(
        output=_sanitize_denial(result.stderr, str(buck.cwd)),
        rel_path="golden/denied_with_context.golden.stderr",
    )


@buck_test()
async def test_agent_host_guard_denied_custom_isolation_dir(buck: Buck) -> None:
    # The remediation command must target the daemon's isolation dir. Run under a
    # non-default isolation dir and check the rejection message accounts for it.
    buck.set_isolation_prefix("custom_iso")
    _write_bsmrconfig_local(
        buck,
        "[bsmr]\nagent_hostname_fail_v2_glob=*\n",
    )
    result = await expect_failure(
        buck.build(
            ":pass",
            env={"BSMR_TEST_DAEMON_ORIGINATING_CGROUP": _AGENT_CGROUP},
        ),
    )
    golden(
        output=_sanitize_denial(result.stderr, str(buck.cwd)),
        rel_path="golden/denied_custom_isolation_dir.golden.stderr",
    )


@buck_test()
async def test_agent_host_guard_non_agent_cgroup(buck: Buck) -> None:
    # Hostname matches but the daemon is not agent-originated -> build succeeds.
    _write_bsmrconfig_local(
        buck,
        "[bsmr]\nagent_hostname_fail_v2_glob=*\n",
    )
    await buck.build(
        ":pass",
        env={"BSMR_TEST_DAEMON_ORIGINATING_CGROUP": _NON_AGENT_CGROUP},
    )


@buck_test()
async def test_agent_host_guard_hostname_no_match(buck: Buck) -> None:
    # Agent cgroup but the hostname glob does not match -> build succeeds.
    _write_bsmrconfig_local(
        buck,
        "[bsmr]\nagent_hostname_fail_v2_glob=definitely-not-this-host-*\n",
    )
    await buck.build(
        ":pass",
        env={"BSMR_TEST_DAEMON_ORIGINATING_CGROUP": _AGENT_CGROUP},
    )
