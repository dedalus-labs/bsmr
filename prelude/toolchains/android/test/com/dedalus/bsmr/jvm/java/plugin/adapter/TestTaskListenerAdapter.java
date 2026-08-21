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

import com.sun.source.util.TaskEvent;
import com.sun.source.util.TaskListener;
import java.util.IdentityHashMap;
import java.util.Map;

public class TestTaskListenerAdapter implements TaskListener {
  public static void addTaskListener(BsmrJavacTask task, TestTaskListener listener) {
    task.addTaskListener(getTaskListener(listener));
  }

  public static void removeTaskListener(BsmrJavacTask task, TestTaskListener listener) {
    task.removeTaskListener(getTaskListener(listener));
  }

  public static void setTaskListener(BsmrJavacTask task, TestTaskListener listener) {
    task.setTaskListener(getTaskListener(listener));
  }

  private static TaskListener getTaskListener(TestTaskListener testTaskListener) {
    if (!taskListeners.containsKey(testTaskListener)) {
      taskListeners.put(testTaskListener, new TestTaskListenerAdapter(testTaskListener));
    }

    return taskListeners.get(testTaskListener);
  }

  private static final Map<TestTaskListener, TaskListener> taskListeners = new IdentityHashMap<>();

  private final TestTaskListener listener;

  private TestTaskListenerAdapter(TestTaskListener listener) {
    this.listener = listener;
  }

  @Override
  public void started(TaskEvent e) {
    listener.started(e.getKind().toString());
  }

  @Override
  public void finished(TaskEvent e) {
    listener.finished(e.getKind().toString());
  }
}
