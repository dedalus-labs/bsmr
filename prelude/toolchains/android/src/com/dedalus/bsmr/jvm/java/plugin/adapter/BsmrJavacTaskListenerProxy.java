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

import com.dedalus.bsmr.jvm.java.plugin.api.BsmrJavacTaskListener;
import com.dedalus.bsmr.jvm.java.plugin.api.CompilationUnitTreeProxy;
import com.dedalus.bsmr.jvm.java.plugin.api.TaskEventMirror;
import com.dedalus.bsmr.util.liteinfersupport.Nullable;
import com.sun.source.tree.CompilationUnitTree;
import com.sun.source.util.TaskEvent;
import com.sun.source.util.TaskListener;

/**
 * Implements {@link TaskListener} by proxying calls to an inner {@link BsmrJavacTaskListener}. This
 * is the bridge that allows us to implement {@link TaskListener}s in Bsmr itself.
 */
public class BsmrJavacTaskListenerProxy implements TaskListener {
  private final BsmrJavacTaskListener bsmrSideListener;

  public BsmrJavacTaskListenerProxy(BsmrJavacTaskListener bsmrSideListener) {
    if (bsmrSideListener instanceof TaskListenerProxy) {
      throw new IllegalArgumentException(
          "taskListener is a proxy, unwrap it rather than creating another proxy");
    }
    this.bsmrSideListener = bsmrSideListener;
  }

  @Override
  public void started(TaskEvent e) {
    this.bsmrSideListener.started(mirrorTaskEvent(e));
  }

  @Override
  public void finished(TaskEvent e) {
    this.bsmrSideListener.finished(mirrorTaskEvent(e));
  }

  private TaskEventMirror mirrorTaskEvent(TaskEvent e) {
    return new TaskEventMirror(
        e,
        mirrorKind(e.getKind()),
        e.getSourceFile(),
        proxyCompilationUnit(e.getCompilationUnit()),
        e.getTypeElement());
  }

  private TaskEventMirror.Kind mirrorKind(TaskEvent.Kind kind) {
    return TaskEventMirror.Kind.valueOf(kind.name());
  }

  @Nullable
  private CompilationUnitTreeProxy proxyCompilationUnit(@Nullable CompilationUnitTree tree) {
    if (tree == null) {
      return null;
    }

    return new CompilationUnitTreeProxyImpl(tree);
  }
}
