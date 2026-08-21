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

import javax.annotation.processing.Messager;
import javax.lang.model.element.AnnotationMirror;
import javax.lang.model.element.AnnotationValue;
import javax.lang.model.element.Element;
import javax.tools.Diagnostic;

class TreeBackedMessager implements Messager {
  private final Messager javacMessager;
  private final FrontendOnlyJavacTask task;

  public TreeBackedMessager(FrontendOnlyJavacTask task, Messager javacMessager) {
    this.task = task;
    this.javacMessager = javacMessager;
  }

  @Override
  public void printMessage(Diagnostic.Kind kind, CharSequence msg) {
    javacMessager.printMessage(kind, msg);
  }

  @Override
  public void printMessage(Diagnostic.Kind kind, CharSequence msg, Element e) {
    javacMessager.printMessage(kind, msg, task.getElements().getJavacElement(e));
  }

  @Override
  public void printMessage(Diagnostic.Kind kind, CharSequence msg, Element e, AnnotationMirror a) {
    javacMessager.printMessage(
        kind, msg, task.getElements().getJavacElement(e), task.getElements().getJavacAnnotation(a));
  }

  @Override
  public void printMessage(
      Diagnostic.Kind kind, CharSequence msg, Element e, AnnotationMirror a, AnnotationValue v) {
    javacMessager.printMessage(
        kind,
        msg,
        task.getElements().getJavacElement(e),
        task.getElements().getJavacAnnotation(a),
        task.getElements().getJavacAnnotationValue(v));
  }
}
