//===----------------------------------------------------------------------===//
// Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc
// Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

package com.dedalus.bsmr.jvm.cd.command.kotlin

import com.dedalus.bsmr.cd.model.kotlin.BuildCommand as ProtoBuildCommand
import com.dedalus.bsmr.core.filesystems.RelPath
import com.dedalus.bsmr.jvm.cd.command.BaseJarCommand
import com.dedalus.bsmr.jvm.cd.command.BuildMode
import com.dedalus.bsmr.jvm.cd.serialization.kotlin.KotlinExtraParamsSerializer
import java.util.Optional

class BuildKotlinCommand(
    val kotlinExtraParams: KotlinExtraParams,
    val baseJarCommand: BaseJarCommand,
    val buildMode: BuildMode,
) {

  companion object {
    fun fromProto(model: ProtoBuildCommand, scratchDir: Optional<RelPath>): BuildKotlinCommand {

      val buildMode: BuildMode = BuildMode.fromProto(model.buildMode)
      val baseJarCommand: BaseJarCommand =
          BaseJarCommand.fromProto(model.baseJarCommand, scratchDir)
      val kotlinExtraParams: KotlinExtraParams =
          KotlinExtraParamsSerializer.deserialize(
              model.baseJarCommand.resolvedJavacOptions,
              model.kotlinExtraParams,
          )

      return BuildKotlinCommand(kotlinExtraParams, baseJarCommand, buildMode)
    }
  }
}
