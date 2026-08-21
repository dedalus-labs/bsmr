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

package com.dedalus.bsmr.jvm.cd.command

import com.dedalus.bsmr.cd.model.java.AbiGenerationMode
import com.dedalus.bsmr.cd.model.java.BaseJarCommand as ProtoBaseJarCommand
import com.dedalus.bsmr.core.filesystems.AbsPath
import com.dedalus.bsmr.core.filesystems.RelPath
import com.dedalus.bsmr.jvm.cd.serialization.AbsPathSerializer
import com.dedalus.bsmr.jvm.cd.serialization.RelPathSerializer
import com.dedalus.bsmr.jvm.cd.serialization.java.BuildTargetValueSerializer
import com.dedalus.bsmr.jvm.cd.serialization.java.CompilerOutputPathsValueSerializer
import com.dedalus.bsmr.jvm.cd.serialization.java.JarParametersSerializer
import com.dedalus.bsmr.jvm.cd.serialization.java.ResolvedJavacOptionsSerializer
import com.dedalus.bsmr.jvm.cd.serialization.java.ResolvedJavacSerializer
import com.dedalus.bsmr.jvm.core.BuildTargetValue
import com.dedalus.bsmr.jvm.java.CompilerOutputPathsValue
import com.dedalus.bsmr.jvm.java.JarParameters
import com.dedalus.bsmr.jvm.java.ResolvedJavac
import com.dedalus.bsmr.jvm.java.ResolvedJavacOptions
import com.google.common.collect.ImmutableList
import com.google.common.collect.ImmutableMap
import com.google.common.collect.ImmutableSortedSet
import java.util.Optional

class BaseJarCommand(
    val abiCompatibilityMode: AbiGenerationMode,
    val abiGenerationMode: AbiGenerationMode,
    val isRequiredForSourceOnlyAbi: Boolean,
    val trackClassUsage: Boolean,
    val compilerOutputPathsValue: CompilerOutputPathsValue,
    val compileTimeClasspathPaths: ImmutableList<RelPath>,
    val compileTimeClasspathSnapshotPathsMap: ImmutableList<RelPath>,
    val javaSrcs: ImmutableSortedSet<RelPath>,
    val resourcesMap: ImmutableMap<RelPath, RelPath>,
    val jarParameters: JarParameters?,
    val buildCellRootPath: AbsPath,
    val resolvedJavac: ResolvedJavac,
    val resolvedJavacOptions: ResolvedJavacOptions,
    val buildTargetValue: BuildTargetValue,
    val bsmrOut: RelPath,
    val pathToClasses: RelPath?,
    val annotationPath: RelPath?,
) {

  companion object {
    fun fromProto(model: ProtoBaseJarCommand, scratchDir: Optional<RelPath>): BaseJarCommand {
      return BaseJarCommand(
          model.abiCompatibilityMode,
          model.abiGenerationMode,
          model.trackClassUsage,
          model.trackClassUsage,
          CompilerOutputPathsValueSerializer.deserialize(model.outputPathsValue, scratchDir),
          RelPathSerializer.toListOfRelPath(model.compileTimeClasspathPathsList),
          RelPathSerializer.toListOfRelPath(model.compileTimeClasspathSnapshotPathsList),
          RelPathSerializer.toSortedSetOfRelPath(model.getJavaSrcsList()),
          RelPathSerializer.toResourceMap(model.resourcesMapList),
          if (model.hasJarParameters()) JarParametersSerializer.deserialize(model.jarParameters)
          else null,
          AbsPathSerializer.deserialize(""),
          ResolvedJavacSerializer.deserialize(model.resolvedJavac),
          ResolvedJavacOptionsSerializer.deserialize(model.resolvedJavacOptions),
          BuildTargetValueSerializer.deserialize(model.buildTargetValue),
          RelPathSerializer.deserialize(model.configuredOutput),
          RelPathSerializer.deserialize(model.pathToClasses),
          RelPathSerializer.deserialize(model.annotationsPath),
      )
    }
  }
}
