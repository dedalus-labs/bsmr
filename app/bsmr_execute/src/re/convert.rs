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

use remote_execution as RE;

pub fn platform_to_proto(platform: &RE::Platform) -> bsmr_data::RePlatform {
    bsmr_data::RePlatform {
        properties: platform
            .properties
            .iter()
            .map(|property| bsmr_data::re_platform::Property {
                name: property.name.clone(),
                value: property.value.clone(),
            })
            .collect(),
    }
}
