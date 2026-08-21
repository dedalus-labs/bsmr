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

import java.util.LinkedHashSet;
import java.util.Set;
import java.util.stream.Collector;
import java.util.stream.Collectors;
import javax.annotation.processing.Completion;
import javax.annotation.processing.ProcessingEnvironment;
import javax.annotation.processing.Processor;
import javax.annotation.processing.RoundEnvironment;
import javax.lang.model.SourceVersion;
import javax.lang.model.element.AnnotationMirror;
import javax.lang.model.element.Element;
import javax.lang.model.element.ExecutableElement;
import javax.lang.model.element.TypeElement;

/**
 * Wraps an annotation processor to ensure that it always sees canonical elements -- that is, {@link
 * TreeBackedElement}s when they are available, javac elements when they are not.
 *
 * <p>Annotation processors that depend on compiler internals or {@link com.sun.source.util.Trees}
 * will not run properly (typically they will crash) when run inside this wrapper.
 */
class TreeBackedProcessorWrapper implements Processor {
  private final Processor inner;
  private final FrontendOnlyJavacTask task;

  TreeBackedProcessorWrapper(FrontendOnlyJavacTask task, Processor inner) {
    this.task = task;
    this.inner = inner;
  }

  @Override
  public Set<String> getSupportedOptions() {
    return inner.getSupportedOptions();
  }

  @Override
  public Set<String> getSupportedAnnotationTypes() {
    return inner.getSupportedAnnotationTypes();
  }

  @Override
  public SourceVersion getSupportedSourceVersion() {
    return inner.getSupportedSourceVersion();
  }

  @Override
  public synchronized void init(ProcessingEnvironment processingEnv) {
    inner.init(new TreeBackedProcessingEnvironment(task, processingEnv));
  }

  @Override
  public Iterable<? extends Completion> getCompletions(
      Element element, AnnotationMirror annotation, ExecutableElement member, String userText) {
    // This method is only ever called from IDEs, which is not a scenario for Bsmr right now
    throw new UnsupportedOperationException();
  }

  @Override
  public boolean process(Set<? extends TypeElement> annotations, RoundEnvironment roundEnv) {
    return inner.process(
        annotations.stream().map(task.getElements()::getCanonicalElement).collect(toSet()),
        new TreeBackedRoundEnvironment(task, roundEnv));
  }

  private Collector<TypeElement, ?, Set<TypeElement>> toSet() {
    return Collectors.toCollection(LinkedHashSet::new);
  }
}
