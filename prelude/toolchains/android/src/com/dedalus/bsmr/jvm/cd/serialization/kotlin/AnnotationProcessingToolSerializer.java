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

package com.dedalus.bsmr.jvm.cd.serialization.kotlin;

import com.dedalus.bsmr.jvm.cd.command.kotlin.AnnotationProcessingTool;
import com.facebook.infer.annotation.Nullsafe;

/**
 * Marshalling between:
 *
 * <ul>
 *   <li>{@link com.dedalus.bsmr.jvm.cd.command.kotlin.AnnotationProcessingTool}, and
 *   <li>{@link com.dedalus.bsmr.cd.model.kotlin.AnnotationProcessingTool} (part of the protocol
 *       buffer model).
 * </ul>
 */
@Nullsafe(Nullsafe.Mode.LOCAL)
public class AnnotationProcessingToolSerializer {

  private AnnotationProcessingToolSerializer() {}

  /** Protocol buffer model to internal bsmr representation. */
  public static AnnotationProcessingTool deserialize(
      com.dedalus.bsmr.cd.model.kotlin.AnnotationProcessingTool annotationProcessingTool) {
    switch (annotationProcessingTool) {
      case KAPT:
        return AnnotationProcessingTool.KAPT;

      case JAVAC:
        return AnnotationProcessingTool.JAVAC;

      case UNRECOGNIZED:
      default:
        throw new IllegalArgumentException(
            "Unrecognised annotation processing tool: " + annotationProcessingTool);
    }
  }
}
