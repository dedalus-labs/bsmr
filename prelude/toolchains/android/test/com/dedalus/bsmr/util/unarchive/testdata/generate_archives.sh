#!/bin/sh
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

set -e
set -x
mkdir -p archive_temp archive_temp/root archive_temp/root_sibling archive_temp/root/src/com/dedalus/bsmr archive_temp/root/alternative
echo "class Main { public static void main(String[] args) { return; } }" > archive_temp/root/src/com/dedalus/bsmr/Main.java
echo "class Other { public static void main(String[] args) { return; } }" > archive_temp/root_sibling/Other.java
pushd archive_temp/root/alternative
ln -s ../src/com/dedalus/bsmr/Main.java Link.java
ln -s Link.java Main.java
popd
echo "#!/bin/sh" > archive_temp/root/echo.sh
echo "echo 'testing'" >> archive_temp/root/echo.sh
chmod u+x archive_temp/root/echo.sh
mkdir archive_temp/root/empty_dir
pushd archive_temp
for i in ".tar cvf" ".tar.bz2 cjvf" ".tar.gz czvf" ".tar.xz cJvf"; do
  extension=$(echo $i | awk {'print $1'})
  args=$(echo $i | awk {'print $2'})
  gtar $args ../output${extension} *
done
popd
rm -rf archive_temp
