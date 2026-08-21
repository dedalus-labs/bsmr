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

import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.verify;

import com.sun.source.util.TaskEvent;
import com.sun.source.util.TaskListener;
import org.junit.Test;

public class TaskListenerWrapperTest {
  private final TaskListener innerListener = mock(TaskListener.class);

  @Test
  public void testChainsToInnerOnStart() {
    TaskListenerWrapper wrapper = new TaskListenerWrapper(innerListener);
    TaskEvent e = new TaskEvent(TaskEvent.Kind.PARSE);

    wrapper.started(e);

    verify(innerListener).started(e);
  }

  @Test
  public void testChainsToInnerOnFinish() {
    TaskListenerWrapper wrapper = new TaskListenerWrapper(innerListener);
    TaskEvent e = new TaskEvent(TaskEvent.Kind.PARSE);

    wrapper.finished(e);

    verify(innerListener).finished(e);
  }

  @Test
  public void testIgnoresNullListeners() {
    TaskListenerWrapper wrapper = new TaskListenerWrapper(null);
    TaskEvent e = new TaskEvent(TaskEvent.Kind.PARSE);

    wrapper.started(e);
    wrapper.finished(e);

    // Expect no crashes
  }
}
