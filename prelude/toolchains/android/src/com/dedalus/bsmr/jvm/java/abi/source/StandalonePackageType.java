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

package com.dedalus.bsmr.jvm.java.abi.source;

import java.util.List;
import javax.lang.model.element.AnnotationMirror;
import javax.lang.model.type.NoType;
import javax.lang.model.type.TypeKind;

public class StandalonePackageType extends StandaloneTypeMirror implements NoType {

  private final ArtificialPackageElement packageElement;

  public StandalonePackageType(
      ArtificialPackageElement packageElement, List<? extends AnnotationMirror> annotations) {
    super(TypeKind.PACKAGE, annotations);
    this.packageElement = packageElement;
  }

  public StandalonePackageType(ArtificialPackageElement packageElement) {
    super(TypeKind.PACKAGE);
    this.packageElement = packageElement;
  }

  public ArtificialPackageElement asElement() {
    return packageElement;
  }

  @Override
  public String toString() {
    return packageElement.getQualifiedName().toString();
  }

  @Override
  public StandalonePackageType cloneWithAnnotationMirrors(
      List<? extends AnnotationMirror> annotations) {
    return new StandalonePackageType(this.packageElement, annotations);
  }
}
