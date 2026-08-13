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

use bsmr_test_api::data::ExternalRunnerSpec;
use bsmr_test_api::protocol::TestExecutor;
use futures::channel::mpsc::UnboundedSender;

pub type SpecSender = UnboundedSender<ExternalRunnerSpec>;

pub struct BsmrTestExecutor {
    pub sender: SpecSender,
}

fn send_spec<T>(sender: &UnboundedSender<T>, spec: T) -> bsmr_error::Result<()> {
    sender
        .clone()
        .start_send(spec)
        .map_err(|_| bsmr_error::internal_error!("test executor spec receiver disconnected"))
}

impl BsmrTestExecutor {
    pub fn new(sender: SpecSender) -> Self {
        Self { sender }
    }
}

#[async_trait::async_trait]
impl TestExecutor for BsmrTestExecutor {
    async fn external_runner_spec(&self, spec: ExternalRunnerSpec) -> bsmr_error::Result<()> {
        send_spec(&self.sender, spec)
    }

    async fn end_of_test_requests(&self) -> bsmr_error::Result<()> {
        // This ensures that all senders are dropped so the receiver can terminate
        self.sender.close_channel();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use futures::channel::mpsc;

    use super::send_spec;

    #[test]
    fn invariant_disconnected_receiver_returns_an_error() {
        let (sender, receiver) = mpsc::unbounded::<()>();
        drop(receiver);

        assert!(send_spec(&sender, ()).is_err());
    }
}
