use std::collections::HashMap;

#[cfg(test)]
use aries_vcx::agency_client::testing::mocking::AgencyMockDecrypted;
use aries_vcx::{
    handlers::{
        issuance::holder::Holder,
        util::{matches_opt_thread_id, matches_thread_id},
    },
    messages::{
        msg_fields::protocols::{
            cred_issuance::{
                v1::{offer_credential::OfferCredentialV1, CredentialIssuanceV1},
                CredentialIssuance,
            },
            notification::Notification,
        },
        AriesMessage,
    },
    protocols::issuance::holder::state_machine::HolderState,
};
use serde_json;
#[cfg(test)]
use test_utils::{
    constants::GET_MESSAGES_DECRYPTED_RESPONSE, mockdata::mockdata_credex::ARIES_CREDENTIAL_OFFER,
};

use crate::{
    api_vcx::{
        api_global::profile::{get_main_anoncreds, get_main_ledger_read, get_main_wallet},
        api_handle::{
            mediated_connection::{self, send_message},
            object_cache::ObjectCache,
            ToU32,
        },
    },
    errors::error::{LibvcxError, LibvcxErrorKind, LibvcxResult},
};

lazy_static! {
    static ref HANDLE_MAP: ObjectCache<Holder> = ObjectCache::<Holder>::new("credentials-cache");
}

// This enum is left only to avoid making breaking serialization changes
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "version", content = "data")]
enum Credentials {
    #[serde(rename = "2.0")]
    V3(Holder),
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Credential {}

fn create_credential(source_id: &str, offer: &str) -> LibvcxResult<Option<Holder>> {
    trace!(
        "create_credential >>> source_id: {}, offer: {}",
        source_id,
        secret!(&offer)
    );

    let offer_message = serde_json::from_str::<serde_json::Value>(offer).map_err(|err| {
        LibvcxError::from_msg(
            LibvcxErrorKind::InvalidJson,
            format!("Cannot deserialize Message: {:?}", err),
        )
    })?;

    let offer_message = match offer_message {
        serde_json::Value::Array(_) => {
            return Err(LibvcxError::from_msg(
                LibvcxErrorKind::InvalidJson,
                "Received offer in legacy format",
            ));
        }
        offer => offer,
    };

    if let Ok(cred_offer) = serde_json::from_value::<OfferCredentialV1>(offer_message) {
        return Ok(Some(Holder::create_from_offer(source_id, cred_offer)?));
    }

    // TODO: Return error in case of error
    Ok(None)
}

pub fn credential_create_with_offer(source_id: &str, offer: &str) -> LibvcxResult<u32> {
    trace!(
        "credential_create_with_offer >>> source_id: {}, offer: {}",
        source_id,
        secret!(&offer)
    );

    let cred_offer: OfferCredentialV1 = serde_json::from_str(offer).map_err(|err| {
        LibvcxError::from_msg(
            LibvcxErrorKind::InvalidJson,
            format!(
                "Strict `aries` protocol is enabled. Can not parse `aries` formatted Credential \
                 Offer: {}",
                err
            ),
        )
    })?;

    let holder = Holder::create_from_offer(source_id, cred_offer)?;
    HANDLE_MAP.add(holder)
}

pub async fn credential_create_with_msgid(
    source_id: &str,
    connection_handle: u32,
    msg_id: &str,
) -> LibvcxResult<(u32, String)> {
    trace!(
        "credential_create_with_msgid >>> source_id: {}, connection_handle: {}, msg_id: {}",
        source_id,
        connection_handle,
        secret!(&msg_id)
    );

    let offer = get_credential_offer_msg(connection_handle, msg_id).await?;
    trace!(
        "credential_create_with_msgid ::: for msg_id {} found offer {}",
        msg_id,
        offer
    );

    let credential = create_credential(source_id, &offer)?.ok_or(LibvcxError::from_msg(
        LibvcxErrorKind::InvalidCredentialHandle,
        "Connection can not be used for Proprietary Issuance protocol",
    ))?;

    let handle = HANDLE_MAP.add(credential)?;

    debug!("inserting credential {} into handle map", source_id);
    Ok((handle, offer))
}

pub fn holder_find_message_to_handle(
    sm: &Holder,
    messages: HashMap<String, AriesMessage>,
) -> Option<(String, AriesMessage)> {
    trace!("holder_find_message_to_handle >>>");
    for (uid, message) in messages {
        match sm.get_state() {
            HolderState::ProposalSet => {
                if let AriesMessage::CredentialIssuance(CredentialIssuance::V1(
                    CredentialIssuanceV1::OfferCredential(offer),
                )) = &message
                {
                    if matches_opt_thread_id!(offer, sm.get_thread_id().unwrap().as_str()) {
                        return Some((uid, message));
                    }
                }
            }
            HolderState::RequestSet => match &message {
                AriesMessage::CredentialIssuance(CredentialIssuance::V1(
                    CredentialIssuanceV1::IssueCredential(credential),
                )) => {
                    if matches_thread_id!(credential, sm.get_thread_id().unwrap().as_str()) {
                        return Some((uid, message));
                    }
                }
                AriesMessage::CredentialIssuance(CredentialIssuance::V1(
                    CredentialIssuanceV1::ProblemReport(problem_report),
                )) => {
                    if matches_opt_thread_id!(problem_report, sm.get_thread_id().unwrap().as_str())
                    {
                        return Some((uid, message));
                    }
                }
                AriesMessage::ReportProblem(problem_report) => {
                    if matches_opt_thread_id!(problem_report, sm.get_thread_id().unwrap().as_str())
                    {
                        return Some((uid, message));
                    }
                }
                AriesMessage::Notification(Notification::ProblemReport(msg)) => {
                    if matches_opt_thread_id!(msg, sm.get_thread_id().unwrap().as_str()) {
                        return Some((uid, message));
                    }
                }
                _ => {}
            },
            _ => {}
        };
    }
    None
}

pub async fn update_state(
    credential_handle: u32,
    message: Option<&str>,
    connection_handle: u32,
) -> LibvcxResult<u32> {
    let mut credential = HANDLE_MAP.get_cloned(credential_handle)?;

    trace!("credential::update_state >>> ");
    if credential.is_terminal_state() {
        return Ok(credential.get_state().to_u32());
    }
    let (mediator_uid, aries_msg) = if let Some(message) = message {
        let message: AriesMessage = serde_json::from_str(message).map_err(|err| {
            LibvcxError::from_msg(
                LibvcxErrorKind::InvalidOption,
                format!(
                    "Cannot update state: Message deserialization failed: {:?}",
                    err
                ),
            )
        })?;
        (None, Some(message))
    } else {
        let messages = mediated_connection::get_messages(connection_handle).await?;
        match holder_find_message_to_handle(&credential, messages) {
            None => (None, None),
            Some((uid, msg)) => (Some(uid), Some(msg)),
        }
    };
    match aries_msg {
        None => {
            trace!(
                "credential::update_state >>> no suitable messages found to progress the protocol"
            );
        }
        Some(aries_msg) => {
            credential
                .process_aries_msg(
                    get_main_wallet()?.as_ref(),
                    get_main_ledger_read()?.as_ref(),
                    get_main_anoncreds()?.as_ref(),
                    aries_msg.clone(),
                )
                .await?;
            if let Some(uid) = mediator_uid {
                trace!("credential::update_state >>> updating messages status in mediator");
                mediated_connection::update_message_status(connection_handle, &uid).await?;
            }
            match credential.get_final_message()? {
                None => {}
                Some(msg_response) => {
                    send_message(connection_handle, msg_response).await?;
                }
            }
        }
    }
    let state = credential.get_state().to_u32();
    HANDLE_MAP.insert(credential_handle, credential)?;
    Ok(state)
}

pub fn get_credential(handle: u32) -> LibvcxResult<String> {
    HANDLE_MAP.get(handle, |credential| {
        Ok(json!(credential.get_credential()?.1).to_string())
    })
}

pub fn get_attributes(handle: u32) -> LibvcxResult<String> {
    HANDLE_MAP.get(handle, |credential| {
        credential.get_attributes().map_err(|err| err.into())
    })
}

pub fn get_attachment(handle: u32) -> LibvcxResult<String> {
    HANDLE_MAP.get(handle, |credential| {
        credential.get_attachment().map_err(|err| err.into())
    })
}

pub fn get_tails_location(handle: u32) -> LibvcxResult<String> {
    HANDLE_MAP.get(handle, |credential| {
        credential.get_tails_location().map_err(|err| err.into())
    })
}

pub fn get_tails_hash(handle: u32) -> LibvcxResult<String> {
    HANDLE_MAP.get(handle, |credential| {
        credential.get_tails_hash().map_err(|err| err.into())
    })
}

pub fn get_rev_reg_id(handle: u32) -> LibvcxResult<String> {
    HANDLE_MAP.get(handle, |credential| {
        credential.get_rev_reg_id().map_err(|err| err.into())
    })
}

pub async fn is_revokable(handle: u32) -> LibvcxResult<bool> {
    let credential = HANDLE_MAP.get_cloned(handle)?;
    credential
        .is_revokable(get_main_ledger_read()?.as_ref())
        .await
        .map_err(|err| err.into())
}

pub async fn delete_credential(handle: u32) -> LibvcxResult<()> {
    trace!(
        "Credential::delete_credential >>> credential_handle: {}",
        handle
    );
    let credential = HANDLE_MAP.get_cloned(handle)?;
    credential
        .delete_credential(get_main_wallet()?.as_ref(), get_main_anoncreds()?.as_ref())
        .await?;
    HANDLE_MAP.release(handle)
}

pub fn get_state(handle: u32) -> LibvcxResult<u32> {
    HANDLE_MAP.get(handle, |credential| Ok(credential.get_state().to_u32()))
}

pub fn generate_credential_request_msg(
    _handle: u32,
    _my_pw_did: &str,
    _their_pw_did: &str,
) -> LibvcxResult<String> {
    Err(LibvcxError::from_msg(
        LibvcxErrorKind::ActionNotSupported,
        "This action is not implemented yet",
    ))
    // TODO: implement
}

pub async fn send_credential_request(handle: u32, connection_handle: u32) -> LibvcxResult<()> {
    trace!(
        "Credential::send_credential_request >>> credential_handle: {}, connection_handle: {}",
        handle,
        connection_handle
    );
    let mut credential = HANDLE_MAP.get_cloned(handle)?;
    let my_pw_did = mediated_connection::get_pw_did(connection_handle)?;
    let msg_response = credential
        .prepare_credential_request(
            get_main_wallet()?.as_ref(),
            get_main_ledger_read()?.as_ref(),
            get_main_anoncreds()?.as_ref(),
            my_pw_did.parse()?,
        )
        .await?;
    send_message(connection_handle, msg_response).await?;
    HANDLE_MAP.insert(handle, credential)
}

async fn get_credential_offer_msg(connection_handle: u32, msg_id: &str) -> LibvcxResult<String> {
    trace!(
        "get_credential_offer_msg >>> connection_handle: {}, msg_id: {}",
        connection_handle,
        msg_id
    );

    let credential_offer =
        match mediated_connection::get_message_by_id(connection_handle, msg_id).await {
            Ok(message) => match message {
                AriesMessage::CredentialIssuance(CredentialIssuance::V1(
                    CredentialIssuanceV1::OfferCredential(_),
                )) => Ok(message),
                msg => {
                    return Err(LibvcxError::from_msg(
                        LibvcxErrorKind::InvalidMessages,
                        format!("Message of different type was received: {:?}", msg),
                    ));
                }
            },
            Err(err) => Err(err),
        }?;

    serde_json::to_string(&credential_offer).map_err(|err| {
        LibvcxError::from_msg(
            LibvcxErrorKind::InvalidState,
            format!("Cannot serialize Offers: {:?}", err),
        )
    })
}

pub async fn get_credential_offer_messages_with_conn_handle(
    connection_handle: u32,
) -> LibvcxResult<String> {
    trace!(
        "Credential::get_credential_offer_messages_with_conn_handle >>> connection_handle: {}",
        connection_handle
    );

    #[cfg(test)]
    {
        AgencyMockDecrypted::set_next_decrypted_response(GET_MESSAGES_DECRYPTED_RESPONSE);
        AgencyMockDecrypted::set_next_decrypted_message(ARIES_CREDENTIAL_OFFER);
    }

    let credential_offers: Vec<AriesMessage> = mediated_connection::get_messages(connection_handle)
        .await?
        .into_iter()
        .filter_map(|(_, a2a_message)| match a2a_message {
            AriesMessage::CredentialIssuance(CredentialIssuance::V1(
                CredentialIssuanceV1::OfferCredential(_),
            )) => Some(a2a_message),
            _ => None,
        })
        .collect();

    Ok(json!(credential_offers).to_string())
}

pub fn release(handle: u32) -> LibvcxResult<()> {
    HANDLE_MAP
        .release(handle)
        .map_err(|e| LibvcxError::from_msg(LibvcxErrorKind::InvalidCredentialHandle, e.to_string()))
}

pub fn release_all() {
    HANDLE_MAP.drain().ok();
}

pub fn is_valid_handle(handle: u32) -> bool {
    HANDLE_MAP.has_handle(handle)
}

pub fn to_string(handle: u32) -> LibvcxResult<String> {
    HANDLE_MAP.get(handle, |credential| {
        serde_json::to_string(&Credentials::V3(credential.clone())).map_err(|err| {
            LibvcxError::from_msg(
                LibvcxErrorKind::InvalidState,
                format!("cannot serialize Credential credentialect: {:?}", err),
            )
        })
    })
}

pub fn get_source_id(handle: u32) -> LibvcxResult<String> {
    HANDLE_MAP
        .get(handle, |credential| Ok(credential.get_source_id()))
        .map_err(|e| LibvcxError::from_msg(LibvcxErrorKind::InvalidCredentialHandle, e.to_string()))
}

pub fn from_string(credential_data: &str) -> LibvcxResult<u32> {
    let credential: Credentials = serde_json::from_str(credential_data).map_err(|err| {
        LibvcxError::from_msg(
            LibvcxErrorKind::InvalidJson,
            format!("Cannot deserialize Credential: {:?}", err),
        )
    })?;

    match credential {
        Credentials::V3(credential) => HANDLE_MAP.add(credential),
    }
}

pub fn is_payment_required(_handle: u32) -> LibvcxResult<bool> {
    Ok(false)
}

pub fn get_credential_status(handle: u32) -> LibvcxResult<u32> {
    HANDLE_MAP.get(handle, |credential| {
        credential.get_credential_status().map_err(|err| err.into())
    })
}

pub fn get_thread_id(handle: u32) -> LibvcxResult<String> {
    HANDLE_MAP.get(handle, |credential| {
        credential.get_thread_id().map_err(|err| err.into())
    })
}

pub async fn decline_offer(
    handle: u32,
    connection_handle: u32,
    comment: Option<&str>,
) -> LibvcxResult<()> {
    let mut credential = HANDLE_MAP.get_cloned(handle)?;
    let problem_report = credential.decline_offer(comment)?;
    send_message(connection_handle, problem_report.into()).await?;
    HANDLE_MAP.insert(handle, credential)
}

pub mod tests_utils {
    pub const BAD_CREDENTIAL_OFFER: &str = r#"{"version": "0.1","to_did": "LtMgSjtFcyPwenK9SHCyb8","from_did": "LtMgSjtFcyPwenK9SHCyb8","claim": {"account_num": ["8BEaoLf8TBmK4BUyX8WWnA"],"name_on_account": ["Alice"]},"schema_seq_no": 48,"issuer_did": "Pd4fnFtRBcMKRVC2go5w3j","claim_name": "Account Certificate","claim_id": "3675417066","msg_ref_id": "ymy5nth"}"#;
}

#[cfg(test)]

mod tests {
    use aries_vcx::{
        messages::msg_fields::protocols::cred_issuance::v1::issue_credential::IssueCredentialV1,
        protocols::issuance::holder::state_machine::HolderState,
    };
    use test_utils::{
        devsetup::SetupMocks,
        mockdata::{
            mockdata_credex,
            mockdata_credex::{
                ARIES_CREDENTIAL_OFFER, ARIES_CREDENTIAL_OFFER_JSON_FORMAT, CREDENTIAL_SM_FINISHED,
            },
        },
    };

    use super::*;
    use crate::api_vcx::api_handle::{
        credential::{
            credential_create_with_offer, get_attributes, get_credential,
            tests_utils::BAD_CREDENTIAL_OFFER,
        },
        mediated_connection::test_utils::build_test_connection_invitee_completed,
    };

    async fn _get_offer(handle: u32) -> String {
        let offers = get_credential_offer_messages_with_conn_handle(handle)
            .await
            .unwrap();
        let offers: serde_json::Value = serde_json::from_str(&offers).unwrap();
        serde_json::to_string(&offers[0]).unwrap()
    }

    #[test]
    fn test_vcx_credential_release() {
        let _setup = SetupMocks::init();
        let handle = credential_create_with_offer(
            "test_credential_create_with_offer",
            ARIES_CREDENTIAL_OFFER,
        )
        .unwrap();
        release(handle).unwrap();
        assert_eq!(
            to_string(handle).unwrap_err().kind,
            LibvcxErrorKind::InvalidHandle
        );
    }

    #[tokio::test]
    async fn test_credential_create_with_offer() {
        let _setup = SetupMocks::init();

        let handle = credential_create_with_offer(
            "test_credential_create_with_offer",
            ARIES_CREDENTIAL_OFFER,
        )
        .unwrap();
        assert!(handle > 0);
    }

    #[tokio::test]
    async fn test_credential_create_with_offer_with_json_attach() {
        let _setup = SetupMocks::init();

        let handle = credential_create_with_offer(
            "test_credential_create_with_offer",
            ARIES_CREDENTIAL_OFFER_JSON_FORMAT,
        )
        .unwrap();
        assert!(handle > 0);
    }

    #[tokio::test]
    async fn test_credential_create_with_bad_offer() {
        let _setup = SetupMocks::init();

        let err = credential_create_with_offer(
            "test_credential_create_with_bad_offer",
            BAD_CREDENTIAL_OFFER,
        )
        .unwrap_err();
        assert_eq!(err.kind(), LibvcxErrorKind::InvalidJson);
    }

    #[tokio::test]
    async fn test_credential_serialize_deserialize() {
        let _setup = SetupMocks::init();

        let handle1 = credential_create_with_offer(
            "test_credential_serialize_deserialize",
            ARIES_CREDENTIAL_OFFER,
        )
        .unwrap();
        let cred_original_state = get_state(handle1).unwrap();
        let cred_original_serialized = to_string(handle1).unwrap();
        release(handle1).unwrap();

        let handle2 = from_string(&cred_original_serialized).unwrap();
        let cred_restored_serialized = to_string(handle2).unwrap();
        let cred_restored_state = get_state(handle2).unwrap();

        assert_eq!(cred_original_state, cred_restored_state);
        assert_eq!(cred_original_serialized, cred_restored_serialized);
    }

    #[tokio::test]
    async fn test_get_attributes_json_attach() {
        let _setup = SetupMocks::init();

        let handle_cred =
            credential_create_with_offer("TEST_CREDENTIAL", ARIES_CREDENTIAL_OFFER_JSON_FORMAT)
                .unwrap();
        assert_eq!(
            HolderState::OfferReceived as u32,
            get_state(handle_cred).unwrap()
        );

        let offer_attrs: String = get_attributes(handle_cred).unwrap();
        let offer_attrs: serde_json::Value = serde_json::from_str(&offer_attrs).unwrap();
        let offer_attrs_expected: serde_json::Value =
            serde_json::from_str(mockdata_credex::OFFERED_ATTRIBUTES).unwrap();
        assert_eq!(offer_attrs, offer_attrs_expected);
    }

    #[tokio::test]
    async fn test_get_credential_offer() {
        let _setup = SetupMocks::init();

        let connection_h = build_test_connection_invitee_completed();

        let offer = get_credential_offer_messages_with_conn_handle(connection_h)
            .await
            .unwrap();
        let o: serde_json::Value = serde_json::from_str(&offer).unwrap();
        debug!("Serialized credential offer: {:?}", &o[0]);
        let _credential_offer: OfferCredentialV1 = serde_json::from_str(&o[0].to_string()).unwrap();
    }

    #[tokio::test]
    async fn test_get_credential_and_deserialize() {
        let _setup = SetupMocks::init();

        let handle = from_string(CREDENTIAL_SM_FINISHED).unwrap();
        let cred_string: String = get_credential(handle).unwrap();
        let cred_value: serde_json::Value = serde_json::from_str(&cred_string).unwrap();
        let _credential_struct: IssueCredentialV1 =
            serde_json::from_str(cred_value.to_string().as_str()).unwrap();
    }
}
