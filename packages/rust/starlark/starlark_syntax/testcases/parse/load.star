# ===----------------------------------------------------------------------===
# Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# @generated
def declare_maven(item):
  sha = item.get("sha1")
  if sha != None:
    native.maven_jar(name = item["name"], artifact = item["artifact"], sha1 = sha)
  else:
    native.maven_jar(name = item["name"], artifact = item["artifact"])
  native.bind(name = item["bind"], actual = item["actual"])
