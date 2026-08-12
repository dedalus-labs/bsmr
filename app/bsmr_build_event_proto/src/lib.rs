//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Defines stable wire types consumed by build observability integrations.

tonic::include_proto!("bsmr.build.v1");

macro_rules! serde_enum {
    ($module:ident, $ty:ty, $prefix:literal) => {
        mod $module {
            use serde::Deserialize;
            use serde::Deserializer;
            use serde::Serialize;
            use serde::Serializer;

            /// Serializes a generated enum using its stable lowercase name.
            pub(super) fn serialize<S>(value: &i32, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                let value = <$ty>::try_from(*value).map_err(serde::ser::Error::custom)?;
                let value = value
                    .as_str_name()
                    .strip_prefix($prefix)
                    .ok_or_else(|| serde::ser::Error::custom("invalid generated enum prefix"))?;
                value.to_ascii_lowercase().serialize(serializer)
            }

            /// Deserializes a stable lowercase name into a generated enum.
            pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<i32, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                let name = format!("{}{}", $prefix, value.to_ascii_uppercase());
                <$ty>::from_str_name(&name)
                    .map(i32::from)
                    .ok_or_else(|| serde::de::Error::unknown_variant(&value, &[]))
            }
        }
    };
}

serde_enum!(serde_test_outcome, crate::TestOutcome, "TEST_OUTCOME_");
serde_enum!(
    serde_execution_kind,
    crate::ExecutionKind,
    "EXECUTION_KIND_"
);
