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

package com.dedalus.bsmr.util;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNotNull;

import com.google.common.util.concurrent.Runnables;
import java.lang.Thread.State;
import org.junit.Test;

/** Unit tests for {@link Threads} class. */
public class ThreadsTest {

  @Test
  public void testNamedThread() {
    String name = "test";
    Runnable runnable = Runnables.doNothing();

    Thread thread = Threads.namedThread(name, runnable);

    assertNotNull(thread);
    assertFalse(thread.isDaemon());
    assertEquals(State.NEW, thread.getState());
    assertEquals(name, thread.getName());
  }

  @Test(expected = NullPointerException.class)
  public void testNamedThreadWithNullName() {
    Threads.namedThread(null, Runnables.doNothing());
  }

  @Test
  public void testNamedThreadWithNullJob() {
    assertNotNull(Threads.namedThread("test", null));
  }
}
