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

package com.dedalus.bsmr.jvm.java.abi;

import com.dedalus.bsmr.jvm.java.abi.source.api.SourceOnlyAbiRuleInfoFactory.SourceOnlyAbiRuleInfo;
import com.dedalus.bsmr.jvm.java.plugin.api.BsmrJavacTaskListener;
import com.dedalus.bsmr.jvm.java.plugin.api.BsmrJavacTaskProxy;
import com.dedalus.bsmr.jvm.java.plugin.api.PluginClassLoader;
import java.lang.reflect.Constructor;
import java.util.function.Supplier;
import javax.tools.Diagnostic;

public final class SourceBasedAbiStubber {
  public static BsmrJavacTaskListener newValidatingTaskListener(
      PluginClassLoader pluginLoader,
      BsmrJavacTaskProxy task,
      SourceOnlyAbiRuleInfo ruleInfo,
      Supplier<Boolean> errorsExist,
      Diagnostic.Kind messageKind) {
    try {
      Class<?> validatingTaskListenerClass =
          pluginLoader.loadClass(
              "com.dedalus.bsmr.jvm.java.abi.source.ValidatingTaskListener", Object.class);
      Constructor<?> constructor =
          validatingTaskListenerClass.getConstructor(
              BsmrJavacTaskProxy.class,
              SourceOnlyAbiRuleInfo.class,
              Supplier.class,
              Diagnostic.Kind.class);

      return BsmrJavacTaskListener.wrapRealTaskListener(
          pluginLoader, constructor.newInstance(task, ruleInfo, errorsExist, messageKind));
    } catch (ReflectiveOperationException e) {
      throw new RuntimeException(
          "Could not load source-generated ABI validator. Your compiler might not support this. "
              + "If it doesn't, you may need to disable source-based ABI generation.",
          e);
    }
  }

  private SourceBasedAbiStubber() {}
}
