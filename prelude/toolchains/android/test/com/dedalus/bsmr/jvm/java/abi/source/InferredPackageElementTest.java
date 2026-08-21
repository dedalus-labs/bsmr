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

import static org.junit.Assert.assertEquals;

import com.dedalus.bsmr.jvm.java.testutil.compiler.CompilerTreeApiTest;
import com.google.common.base.Joiner;
import java.io.IOException;
import javax.lang.model.element.PackageElement;
import javax.lang.model.element.TypeElement;
import javax.lang.model.type.DeclaredType;
import org.junit.Before;
import org.junit.Test;

public class InferredPackageElementTest extends CompilerTreeApiTest {
  @Before
  public void setUp() {
    testCompiler.useFrontendOnlyJavacTask();
  }

  @Test
  public void testToStringSimpleNamePackage() throws IOException {
    compile(Joiner.on('\n').join("public class Foo extends pkg.Bar.Baz {", "}"));

    DeclaredType superclass = (DeclaredType) elements.getTypeElement("Foo").getSuperclass();
    TypeElement superclassElement = (TypeElement) superclass.asElement();
    TypeElement superclassEnclosingTypeElement =
        (TypeElement) superclassElement.getEnclosingElement();
    PackageElement element =
        (InferredPackageElement) superclassEnclosingTypeElement.getEnclosingElement();

    assertEquals("pkg", element.toString());
  }

  @Test
  public void testToStringQualifiedNamePackage() throws IOException {
    compile(Joiner.on('\n').join("public class Foo extends com.example.foo.Bar.Baz {", "}"));

    DeclaredType superclass = (DeclaredType) elements.getTypeElement("Foo").getSuperclass();
    TypeElement superclassElement = (TypeElement) superclass.asElement();
    TypeElement superclassEnclosingTypeElement =
        (TypeElement) superclassElement.getEnclosingElement();
    PackageElement element =
        (InferredPackageElement) superclassEnclosingTypeElement.getEnclosingElement();

    assertEquals("com.example.foo", element.toString());
  }
}
