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

import com.dedalus.bsmr.jvm.cd.serialization.PathSerializer;
import com.dedalus.bsmr.jvm.java.ActionMetadata;
import com.facebook.infer.annotation.Nullsafe;
import java.nio.file.Path;
import java.util.Map;
import java.util.stream.Collectors;

/**
 * Marshalling between:
 *
 * <ul>
 *   <li>{@link ActionMetadata} (metadata provided by incremental actions, see: <a
 *       href="https://oss.dedaluslabs.ai/bsmr/rule_authors/incremental_actions/">...</a>), and
 *   <li>{@link com.dedalus.bsmr.cd.model.kotlin.ActionMetadata} (part of the protocol buffer
 *       model).
 * </ul>
 */
@Nullsafe(Nullsafe.Mode.LOCAL)
public class ActionMetadataSerializer {

  private ActionMetadataSerializer() {}

  /** Protocol buffer model to internal bsmr representation. */
  public static ActionMetadata deserialize(
      Path incrementalMetadataFilePath,
      com.dedalus.bsmr.cd.model.kotlin.ActionMetadata actionMetadata) {
    Map<Path, String> previousDigest =
        actionMetadata.getPreviousMetadata().getDigestsList().stream()
            .collect(
                Collectors.toMap(
                    digest -> PathSerializer.deserialize(digest.getPath()),
                    com.dedalus.bsmr.cd.model.kotlin.Digests::getDigest));
    Map<Path, String> currentDigest =
        actionMetadata.getCurrentMetadata().getDigestsList().stream()
            .collect(
                Collectors.toMap(
                    digest -> PathSerializer.deserialize(digest.getPath()),
                    com.dedalus.bsmr.cd.model.kotlin.Digests::getDigest));

    return new ActionMetadata(incrementalMetadataFilePath, previousDigest, currentDigest);
  }
}
