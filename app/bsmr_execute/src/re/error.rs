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

use allocative::Allocative;
use bsmr_error::ErrorTag;
use bsmr_error::TypedContext;
use remote_execution::REClientError;
use remote_execution::TCode;
use remote_execution::TCodeReasonGroup;

pub fn get_re_error_tag(tcode: &TCode) -> ErrorTag {
    match *tcode {
        TCode::CANCELLED => ErrorTag::ReCancelled,
        TCode::UNKNOWN => ErrorTag::ReUnknown,
        TCode::INVALID_ARGUMENT => ErrorTag::ReInvalidArgument,
        TCode::DEADLINE_EXCEEDED => ErrorTag::ReDeadlineExceeded,
        TCode::NOT_FOUND => ErrorTag::ReNotFound,
        TCode::ALREADY_EXISTS => ErrorTag::ReAlreadyExists,
        TCode::PERMISSION_DENIED => ErrorTag::RePermissionDenied,
        TCode::RESOURCE_EXHAUSTED => ErrorTag::ReResourceExhausted,
        TCode::FAILED_PRECONDITION => ErrorTag::ReFailedPrecondition,
        TCode::ABORTED => ErrorTag::ReAborted,
        TCode::OUT_OF_RANGE => ErrorTag::ReOutOfRange,
        TCode::UNIMPLEMENTED => ErrorTag::ReUnimplemented,
        TCode::INTERNAL => ErrorTag::ReInternal,
        TCode::UNAVAILABLE => ErrorTag::ReUnavailable,
        TCode::DATA_LOSS => ErrorTag::ReDataLoss,
        TCode::UNAUTHENTICATED => ErrorTag::ReUnauthenticated,
        _ => ErrorTag::ReUnknownTcode,
    }
}

pub fn get_re_group_tag(group: &TCodeReasonGroup) -> Option<ErrorTag> {
    match *group {
        TCodeReasonGroup::RE_CONNECTION => Some(ErrorTag::ReConnection),
        TCodeReasonGroup::USER_QUOTA => Some(ErrorTag::ReUserQuota),
        TCodeReasonGroup::USER_BAD_CERTS => Some(ErrorTag::ReUserBadCerts),
        _ => None,
    }
}

#[derive(Allocative, Debug, Clone, bsmr_error::Error)]
#[error("Remote Execution Error on {} for ReSession {}\nError: ({})", .re_action, .re_session_id, .message)]
#[bsmr(tag = get_re_error_tag(code))]
pub struct RemoteExecutionError {
    re_action: String,
    re_session_id: String,
    pub message: String,
    #[allocative(skip)]
    pub code: TCode,
    #[allocative(skip)]
    pub group: TCodeReasonGroup,
}

impl TypedContext for RemoteExecutionError {
    fn eq(&self, other: &dyn TypedContext) -> bool {
        match (other as &dyn std::any::Any).downcast_ref::<Self>() {
            Some(right) => self.eq(right),
            None => false,
        }
    }

    fn display(&self) -> Option<String> {
        None
    }
}

fn re_error(
    re_action: &str,
    re_session_id: &str,
    message: String,
    code: TCode,
    group: TCodeReasonGroup,
) -> bsmr_error::Error {
    let re_error = RemoteExecutionError {
        re_action: re_action.to_owned(),
        re_session_id: re_session_id.to_owned(),
        message,
        code,
        group,
    };
    let error = bsmr_error::Error::from(re_error.clone()).context(re_error);
    if let Some(tag) = get_re_group_tag(&group) {
        error.tag([tag])
    } else {
        error.string_tag(&group.to_string())
    }
}

pub(crate) async fn with_error_handler<T>(
    re_action: &str,
    re_session_id: &str,
    result: anyhow::Result<T>,
) -> bsmr_error::Result<T> {
    match result {
        Ok(val) => Ok(val),
        Err(e) => {
            let (code, group) = e
                .downcast_ref::<REClientError>()
                .map_or((TCode::UNKNOWN, TCodeReasonGroup::UNKNOWN), |e| {
                    (e.code, e.group)
                });

            Err(re_error(
                re_action,
                re_session_id,
                format!("{e:#}"),
                code,
                group,
            ))
        }
    }
}

pub fn test_re_error(message: &str, code: TCode) -> bsmr_error::Error {
    re_error(
        "test",
        "test",
        message.to_owned(),
        code,
        TCodeReasonGroup::UNKNOWN,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_re_error() {
        let error: bsmr_error::Error = re_error(
            "test",
            "test",
            "test".to_owned(),
            TCode::UNKNOWN,
            TCodeReasonGroup::UNKNOWN,
        );

        let err = error.find_typed_context::<RemoteExecutionError>().unwrap();
        assert_eq!(err.code, TCode::UNKNOWN);
    }
}
