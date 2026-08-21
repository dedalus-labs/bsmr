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

package com.dedalus.bsmr.jvm.java.plugin.adapter;

import com.dedalus.bsmr.jvm.java.lang.model.ElementsExtended;
import com.dedalus.bsmr.jvm.java.plugin.api.BsmrJavacTaskListener;
import com.dedalus.bsmr.jvm.java.plugin.api.BsmrJavacTaskProxy;
import com.dedalus.bsmr.jvm.java.plugin.api.CompilationUnitTreeProxy;
import com.sun.source.util.JavacTask;
import com.sun.source.util.TaskListener;
import java.io.IOException;
import java.util.Locale;
import java.util.Set;
import java.util.function.Consumer;
import java.util.stream.Collectors;
import java.util.stream.StreamSupport;
import javax.annotation.processing.Messager;
import javax.annotation.processing.Processor;
import javax.lang.model.element.Element;
import javax.lang.model.util.Types;
import javax.tools.JavaCompiler;
import javax.tools.JavaFileObject;

/**
 * NOTE: A Java8 copy of this file exists in ../java8/BsmrJavacTaskProxyImpl.java. Please make sure
 * to update the other copy when modifying this file.
 */
public class BsmrJavacTaskProxyImpl implements BsmrJavacTaskProxy {
  private final BsmrJavacTask javacTask;

  public BsmrJavacTaskProxyImpl(JavaCompiler.CompilationTask javacTask) {
    this.javacTask = new BsmrJavacTask((JavacTask) javacTask);
  }

  public BsmrJavacTaskProxyImpl(BsmrJavacTask javacTask) {
    this.javacTask = javacTask;
  }

  public BsmrJavacTask getInner() {
    return javacTask;
  }

  @Override
  public Iterable<CompilationUnitTreeProxy> parse() throws IOException {
    return StreamSupport.stream(javacTask.parse().spliterator(), false)
        .map(CompilationUnitTreeProxyImpl::new)
        .collect(Collectors.toList());
  }

  @Override
  public Iterable<? extends Element> enter() throws IOException {
    return javacTask.enter();
  }

  @Override
  public Iterable<? extends Element> analyze() throws IOException {
    return javacTask.analyze();
  }

  @Override
  public Iterable<? extends JavaFileObject> generate() throws IOException {
    return javacTask.generate();
  }

  @Override
  public void setTaskListener(BsmrJavacTaskListener bsmrTaskListener) {
    javacTask.setTaskListener(getTaskListener(bsmrTaskListener));
  }

  @Override
  public void addTaskListener(BsmrJavacTaskListener bsmrTaskListener) {
    javacTask.addTaskListener(getTaskListener(bsmrTaskListener));
  }

  @Override
  public void removeTaskListener(BsmrJavacTaskListener bsmrTaskListener) {
    javacTask.removeTaskListener(getTaskListener(bsmrTaskListener));
  }

  private TaskListener getTaskListener(BsmrJavacTaskListener taskListener) {
    if (taskListener instanceof TaskListenerProxy) {
      return ((TaskListenerProxy) taskListener).getInner();
    }

    return new BsmrJavacTaskListenerProxy(taskListener);
  }

  @Override
  public void addPostEnterCallback(Consumer<Set<Element>> callback) {
    javacTask.addPostEnterCallback(callback);
  }

  @Override
  public ElementsExtended getElements() {
    return javacTask.getElements();
  }

  @Override
  public Types getTypes() {
    return javacTask.getTypes();
  }

  @Override
  public Messager getMessager() {
    return new TreesMessager(javacTask.getTrees());
  }

  @Override
  public void setProcessors(Iterable<? extends Processor> processors) {
    javacTask.setProcessors(processors);
  }

  @Override
  public void setLocale(Locale locale) {
    javacTask.setLocale(locale);
  }

  @Override
  public Boolean call() {
    return javacTask.call();
  }

  @Override
  public void addModules(Iterable<String> moduleNames) {
    javacTask.addModules(moduleNames);
  }
}
