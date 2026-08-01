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

#![cfg(test)]

use bsmr_data::error::ErrorTag;

use crate as bsmr_error;

#[derive(bsmr_error_derive::Error, Debug)]
#[error("foo")]
#[bsmr(input)]
pub struct Error1;

#[test]
fn test_derive_error1() {
    let e: crate::Error = Error1.into();
    assert_eq!(e.get_tier(), Some(crate::Tier::Input));
}

#[derive(bsmr_error_derive::Error, Debug)]
#[error("foo")]
#[bsmr(tier0)]
#[allow(unused)]
struct Error2((), ());

#[test]
fn test_derive_error2() {
    let e: crate::Error = Error2((), ()).into();
    assert_eq!(e.get_tier(), Some(crate::Tier::Tier0));
}

#[derive(bsmr_error_derive::Error, Debug)]
pub enum Error3 {
    #[error("foo")]
    #[bsmr(input)]
    VariantA,
    #[error("bar")]
    #[bsmr(tier0)]
    VariantB,
    #[error("baz")]
    #[bsmr(tag = Environment)]
    VariantC,
}

#[test]
fn test_derive_error3() {
    let e: crate::Error = Error3::VariantA.into();
    assert_eq!(e.get_tier(), Some(crate::Tier::Input));

    let e: crate::Error = Error3::VariantB.into();
    assert_eq!(e.get_tier(), Some(crate::Tier::Tier0));

    let e: crate::Error = Error3::VariantC.into();
    assert_eq!(e.get_tier(), Some(crate::Tier::Environment));
}

#[derive(bsmr_error_derive::Error, Debug)]
#[error("Generic error")]
#[bsmr(tag = Environment)]
pub struct GenericError<G>(G);

#[test]
fn test_generic_error() {
    let _e: crate::Error = GenericError(42).into();
}

/// Test that no unused fields warning is emitted.
#[derive(bsmr_error_derive::Error, Debug)]
#[error("Unused")]
#[bsmr(tag = Environment)]
pub struct WithField {
    x: u8,
}

#[test]
fn test_with_field() {
    let _e: crate::Error = WithField { x: 42 }.into();
}

#[derive(bsmr_error_derive::Error, Debug)]
#[error("Unused")]
#[bsmr(tag = Environment)]
struct NoAttrsStruct;

#[derive(bsmr_error_derive::Error, Debug)]
#[error("Unused")]
#[bsmr(tag = TestOnly)]
enum NoAttrsEnum {
    Variant,
}

#[test]
fn test_source_location_no_attrs() {
    let e: crate::Error = NoAttrsStruct.into();
    assert!(
        e.source_location()
            .to_string()
            .starts_with("bsmr_error/src/derive_tests.rs::NoAttrsStruct::")
    );
    let e: crate::Error = NoAttrsEnum::Variant.into();
    assert!(
        e.source_location()
            .to_string()
            .starts_with("bsmr_error/src/derive_tests.rs::NoAttrsEnum::Variant::")
    );
}

#[derive(bsmr_error_derive::Error, Debug)]
#[error("Unused")]
#[bsmr(input)]
enum EnumWithTypeOption {
    Variant,
}

#[test]
fn test_enum_with_type_option() {
    let e: crate::Error = EnumWithTypeOption::Variant.into();
    assert_eq!(e.get_tier(), Some(crate::Tier::Input));
    assert!(
        e.source_location()
            .to_string()
            .starts_with("bsmr_error/src/derive_tests.rs::EnumWithTypeOption::Variant::")
    );
}

#[derive(bsmr_error_derive::Error, Debug)]
#[error("Unused")]
#[bsmr(input)]
struct ErrorWithSpelledOutCategory;

#[test]
fn test_error_with_spelled_out_category() {
    let e: crate::Error = ErrorWithSpelledOutCategory.into();
    assert_eq!(e.get_tier(), Some(crate::Tier::Input));
}

#[test]
fn test_source_metadata_are_included() {
    #[derive(bsmr_error_derive::Error, Debug)]
    #[error("WatchmanError")]
    #[bsmr(tag = WatchmanTimeout)]
    struct WatchmanError;

    #[derive(bsmr_error_derive::Error, Debug)]
    #[error("Unused")]
    #[bsmr(tag = WatchmanRequestError)]
    enum MaybeWatchmanError {
        Some(#[source] WatchmanError),
        None,
    }

    let e: crate::Error = MaybeWatchmanError::None.into();
    assert!(e.has_tag(crate::ErrorTag::WatchmanRequestError));

    let e: crate::Error = MaybeWatchmanError::Some(WatchmanError).into();
    assert!(e.has_tag(crate::ErrorTag::WatchmanTimeout));
    assert!(e.has_tag(crate::ErrorTag::WatchmanRequestError));

    assert!(format!("{e:?}").contains("Unused"));
    assert!(format!("{e:?}").contains("WatchmanError"));
}

#[test]
fn test_error_tags() {
    fn f() -> crate::ErrorTag {
        crate::ErrorTag::StarlarkFail
    }

    #[derive(bsmr_error_derive::Error, Debug)]
    #[error("Unused")]
    #[bsmr(tag = WatchmanTimeout)]
    enum TaggedError {
        #[bsmr(tag = f())]
        A,
        #[bsmr(tag = WatchmanTimeout)]
        B,
    }

    let a: crate::Error = TaggedError::A.into();
    assert_eq!(
        &a.tags(),
        &[
            crate::ErrorTag::StarlarkFail,
            crate::ErrorTag::WatchmanTimeout
        ]
    );
    let b: crate::Error = TaggedError::B.into();
    assert_eq!(&b.tags(), &[crate::ErrorTag::WatchmanTimeout]);
}

#[test]
fn test_error_tags_vec_fn() {
    fn calc_tags(extra_tag: bool) -> Vec<ErrorTag> {
        if extra_tag {
            vec![ErrorTag::StarlarkFail]
        } else {
            vec![]
        }
    }

    #[derive(bsmr_error_derive::Error, Debug)]
    #[error("Unused")]
    #[bsmr(tag = WatchmanTimeout, tags = calc_tags(*extra_tag))]
    struct TaggedError {
        extra_tag: bool,
    }

    let a: crate::Error = TaggedError { extra_tag: true }.into();
    assert_eq!(
        &a.tags(),
        &[ErrorTag::StarlarkFail, ErrorTag::WatchmanTimeout]
    );
    let b: crate::Error = TaggedError { extra_tag: false }.into();
    assert_eq!(&b.tags(), &[ErrorTag::WatchmanTimeout]);
}

#[test]
fn test_correct_transparent() {
    #[derive(bsmr_error_derive::Error, Debug)]
    #[error("Unused")]
    #[bsmr(tier0)]
    struct E;

    #[derive(bsmr_error_derive::Error, Debug)]
    #[error(transparent)]
    #[bsmr(tag = Input)]
    struct T(E);

    let t: crate::Error = T(E).into();
    assert_eq!(t.get_tier(), Some(crate::Tier::Tier0));
}

#[test]
fn test_error_message_with_provided_field() {
    #[derive(bsmr_error_derive::Error, Debug)]
    #[error("Some message {0} + {1}")]
    #[bsmr(tag = Environment)]
    struct SomeError(String, String);

    let t: crate::Error = SomeError("test123".to_owned(), "test222".to_owned()).into();
    assert!(format!("{t:?}").contains("Some message test123"));
}

#[test]
fn test_recovery_through_transparent_bsmr_error() {
    #[derive(bsmr_error_derive::Error, Debug)]
    #[error("base_display")]
    #[bsmr(tag = Environment)]
    struct BaseError;

    #[derive(bsmr_error_derive::Error, Debug)]
    #[error(transparent)]
    #[bsmr(tag = TestOnly)]
    enum PartiallyStructured {
        #[error(transparent)]
        Other(bsmr_error::Error),
    }

    let base: crate::Error = crate::Error::from(BaseError).tag([crate::ErrorTag::StarlarkFail]);
    let wrapped_direct: crate::Error = PartiallyStructured::Other(base.clone()).into();

    assert!(format!("{wrapped_direct:?}").contains("base_display"));
    assert_eq!(
        &wrapped_direct.tags()[..],
        &[
            crate::ErrorTag::Environment,
            crate::ErrorTag::StarlarkFail,
            crate::ErrorTag::TestOnly
        ]
    );
}
