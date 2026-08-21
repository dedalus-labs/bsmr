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

package com.dedalus.bsmr.jvm.java.stepsbuilder.javacd.main;

import com.dedalus.bsmr.core.filesystems.AbsPath;
import com.dedalus.bsmr.core.filesystems.RelPath;
import com.dedalus.bsmr.jvm.cd.AbiDirWriter;
import com.dedalus.bsmr.jvm.cd.BuildCommandStepsBuilder;
import com.dedalus.bsmr.jvm.cd.DepFileUtils;
import com.dedalus.bsmr.jvm.cd.JvmCDCommand;
import com.dedalus.bsmr.jvm.cd.command.PostBuildParams;
import com.dedalus.bsmr.jvm.cd.command.java.BuildJavaCommand;
import com.dedalus.bsmr.jvm.java.JavaStepsBuilder;
import com.dedalus.bsmr.jvm.java.abi.StubJar;
import com.google.common.base.Preconditions;
import com.google.common.collect.ImmutableMap;
import com.google.protobuf.util.JsonFormat;
import java.io.FileReader;
import java.io.IOException;
import java.io.Reader;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Optional;
import org.kohsuke.args4j.CmdLineException;
import org.kohsuke.args4j.CmdLineParser;
import org.kohsuke.args4j.Option;

/** Represents a javacd command invocation, on the command line or sent to a worker */
public class JavaCDCommand implements JvmCDCommand {
  @Option(name = "--action-id", required = true)
  public String actionId;

  @Option(name = "--command-file", required = true)
  public Path commandFile;

  @Option(name = "--logging-level")
  private int loggingLevel = 0;

  private final JavaStepsBuilder stepsBuilder;
  private final PostBuildParams postBuildParams;

  public JavaCDCommand(String[] args, ImmutableMap<String, String> env)
      throws CmdLineException, IOException {
    CmdLineParser parser = new CmdLineParser(this);
    try {
      parser.parseArgument(args);
    } catch (CmdLineException e) {
      System.err.println(e.getMessage());
      parser.printUsage(System.err);
      throw e;
    }
    com.dedalus.bsmr.cd.model.java.BuildJavaCommand.Builder builder =
        com.dedalus.bsmr.cd.model.java.BuildJavaCommand.newBuilder();
    try (Reader reader = new FileReader(commandFile.toFile())) {
      JsonFormat.parser().ignoringUnknownFields().merge(reader, builder);
    }
    RelPath bsmrScratchPath = RelPath.get(env.get(WORKING_DIRECTORY_ENV_VAR));

    com.dedalus.bsmr.cd.model.java.BuildJavaCommand proto = builder.build();
    BuildJavaCommand buildJavaCommand =
        BuildJavaCommand.Companion.fromProto(proto.getBuildCommand(), Optional.of(bsmrScratchPath));
    this.postBuildParams = PostBuildParams.Companion.fromProto(proto.getPostBuildParams());
    this.stepsBuilder = new JavaStepsBuilder(buildJavaCommand);
  }

  public void maybeWriteClassAbi() throws IOException {
    if (!postBuildParams.getShouldCreateClassAbi()) {
      return;
    }

    AbsPath root = AbsPath.of(Paths.get(".").toAbsolutePath().normalize());
    Preconditions.checkState(postBuildParams.getAbiJar() != null);
    StubJar stubJar = new StubJar(root.resolve(postBuildParams.getLibraryJar()), false);
    stubJar.writeTo(root.resolve(postBuildParams.getAbiJar()));
  }

  public void maybeWriteAbiDir() throws IOException {
    if (postBuildParams.getAbiOutputDir() == null) {
      Preconditions.checkState(postBuildParams.getAbiJar() == null);
      return;
    }

    if (postBuildParams.getAbiJar() != null) {
      AbiDirWriter.writeAbiOutputDir(
          postBuildParams.getAbiJar(), postBuildParams.getAbiOutputDir());
    } else {
      throw new RuntimeException("Asked for an ABI dir but there is no ABI!");
    }
  }

  public void maybeWriteDepFile() throws IOException {
    Preconditions.checkState(
        (postBuildParams.getDepFile() == null)
            == (postBuildParams.getUsedClassesPaths().isEmpty()));
    if (postBuildParams.getDepFile() != null) {
      DepFileUtils.usedClassesToDepFile(
          postBuildParams.getUsedClassesPaths(),
          postBuildParams.getDepFile(),
          Optional.ofNullable(postBuildParams.getJarToJarDirMap()),
          false);
    }
  }

  public void maybeWriteUsedJarsFile() throws IOException {
    if (postBuildParams.getUsedJarsPath() == null) {
      return;
    }

    DepFileUtils.usedClassesToUsedJars(
        postBuildParams.getUsedClassesPaths(), postBuildParams.getUsedJarsPath(), false);
  }

  @Override
  public BuildCommandStepsBuilder getBuildCommand() {
    return stepsBuilder;
  }

  @Override
  public String getActionId() {
    return actionId;
  }

  @Override
  public int getLoggingLevel() {
    return loggingLevel;
  }

  @Override
  public void postExecute() throws IOException {
    maybeWriteClassAbi();
    maybeWriteAbiDir();
    maybeWriteDepFile();
    maybeWriteUsedJarsFile();
  }
}
