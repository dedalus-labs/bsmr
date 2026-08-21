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

package com.dedalus.bsmr.jvm.kotlin.cd.workertool;

import com.dedalus.bsmr.core.util.log.Logger;
import com.dedalus.bsmr.jvm.cd.CompilerDaemonLoggerUtil;
import com.dedalus.bsmr.jvm.cd.workertool.grpc.WorkerGrpcServer;
import java.io.IOException;

/**
 * KotlinCD grpc worker tool daemon, for use by bsmr
 *
 * <p>This starts a grpc service over a uds socket, that accepts kotlincd compilation commands
 */
public class KotlinCDGrpcWorkerToolMain {
  private static final String LOG_PATH = "bsmr-out/default/kotlincd";

  public static void main(String[] args) throws IOException {
    CompilerDaemonLoggerUtil.setDefaultLogger("kotlincd_grpc_worker", LOG_PATH);
    Logger logger = Logger.get(KotlinCDGrpcWorkerToolMain.class.getName());
    logger.info("Starting KotlinCD Worker GRPC Server");
    WorkerGrpcServer.runServer("KotlinCD", KotlinCDCommand::new);
  }
}
