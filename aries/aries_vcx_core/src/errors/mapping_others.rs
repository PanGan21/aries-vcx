use std::sync::PoisonError;

use did_parser::ParseError;
use public_key::PublicKeyError;

use crate::errors::error::{AriesVcxCoreError, AriesVcxCoreErrorKind};

impl From<serde_json::Error> for AriesVcxCoreError {
    fn from(err: serde_json::Error) -> Self {
        AriesVcxCoreError::from_msg(
            AriesVcxCoreErrorKind::InvalidJson,
            format!("Invalid json: {err}"),
        )
    }
}

impl<T> From<PoisonError<T>> for AriesVcxCoreError {
    fn from(err: PoisonError<T>) -> Self {
        AriesVcxCoreError::from_msg(AriesVcxCoreErrorKind::InvalidState, err.to_string())
    }
}

impl From<ParseError> for AriesVcxCoreError {
    fn from(err: ParseError) -> Self {
        AriesVcxCoreError::from_msg(AriesVcxCoreErrorKind::ParsingError, err.to_string())
    }
}

impl From<Box<dyn std::error::Error>> for AriesVcxCoreError {
    fn from(err: Box<dyn std::error::Error>) -> Self {
        AriesVcxCoreError::from_msg(AriesVcxCoreErrorKind::InvalidState, err.to_string())
    }
}

impl From<anoncreds_types::Error> for AriesVcxCoreError {
    fn from(err: anoncreds_types::Error) -> Self {
        AriesVcxCoreError::from_msg(AriesVcxCoreErrorKind::InvalidState, err.to_string())
    }
}

impl From<PublicKeyError> for AriesVcxCoreError {
    fn from(value: PublicKeyError) -> Self {
        AriesVcxCoreError::from_msg(AriesVcxCoreErrorKind::NotBase58, value)
    }
}
