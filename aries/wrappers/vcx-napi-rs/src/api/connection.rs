use chrono::Utc;
use libvcx_core::{
    api_vcx::api_handle::connection,
    aries_vcx::{
        messages::{
            decorators::timing::Timing,
            msg_fields::protocols::basic_message::{
                BasicMessage, BasicMessageContent, BasicMessageDecorators,
            },
            AriesMessage,
        },
        protocols::connection::pairwise_info::PairwiseInfo,
    },
    errors::error::{LibvcxError, LibvcxErrorKind},
    serde_json,
};
use napi_derive::napi;
use uuid::Uuid;

use crate::error::to_napi_err;

#[napi]
pub async fn connection_create_inviter(pw_info: Option<String>) -> napi::Result<u32> {
    trace!("connection_create_inviter >>>");
    let pw_info = if let Some(pw_info) = pw_info {
        Some(
            serde_json::from_str::<PairwiseInfo>(&pw_info)
                .map_err(|err| {
                    LibvcxError::from_msg(
                        LibvcxErrorKind::InvalidJson,
                        format!("Cannot deserialize pw info: {:?}", err),
                    )
                })
                .map_err(to_napi_err)?,
        )
    } else {
        None
    };
    connection::create_inviter(pw_info)
        .await
        .map_err(to_napi_err)
}

#[napi]
pub async fn connection_create_invitee(invitation: String) -> napi::Result<u32> {
    trace!("connection_create_invitee >>> invitation: {:?}", invitation);
    connection::create_invitee(&invitation)
        .await
        .map_err(to_napi_err)
}

#[napi]
pub fn connection_get_thread_id(handle: u32) -> napi::Result<String> {
    trace!("connection_get_thread_id >>> handle: {:?}", handle);
    connection::get_thread_id(handle).map_err(to_napi_err)
}

#[napi]
pub fn connection_get_pairwise_info(handle: u32) -> napi::Result<String> {
    trace!("connection_get_pairwise_info >>> handle: {:?}", handle);
    connection::get_pairwise_info(handle).map_err(to_napi_err)
}

#[napi]
pub fn connection_get_remote_did(handle: u32) -> napi::Result<String> {
    trace!("connection_get_remote_did >>> handle: {:?}", handle);
    connection::get_remote_did(handle).map_err(to_napi_err)
}

#[napi]
pub fn connection_get_remote_vk(handle: u32) -> napi::Result<String> {
    trace!("connection_get_remote_vk >>> handle: {:?}", handle);
    connection::get_remote_vk(handle).map_err(to_napi_err)
}

#[napi]
pub fn connection_get_state(handle: u32) -> napi::Result<u32> {
    trace!("connection_get_state >>> handle: {:?}", handle);
    connection::get_state(handle).map_err(to_napi_err)
}

#[napi]
pub fn connection_get_invitation(handle: u32) -> napi::Result<String> {
    trace!("connection_get_invitation >>> handle: {:?}", handle);
    connection::get_invitation(handle).map_err(to_napi_err)
}

#[napi]
pub async fn connection_process_invite(handle: u32, invitation: String) -> napi::Result<()> {
    trace!("connection_process_invite >>> handle: {:?}", handle);
    connection::process_invite(handle, &invitation)
        .await
        .map_err(to_napi_err)
}

#[napi]
pub async fn connection_process_request(
    handle: u32,
    request: String,
    service_endpoint: String,
    routing_keys: Vec<String>,
) -> napi::Result<()> {
    trace!("connection_process_request >>> handle: {:?}", handle);
    connection::process_request(handle, &request, service_endpoint, routing_keys)
        .await
        .map_err(to_napi_err)
}

#[napi]
pub async fn connection_process_response(handle: u32, response: String) -> napi::Result<()> {
    trace!("connection_process_response >>> handle: {:?}", handle);
    connection::process_response(handle, &response)
        .await
        .map_err(to_napi_err)
}

#[napi]
pub async fn connection_process_ack(handle: u32, message: String) -> napi::Result<()> {
    trace!("connection_process_ack >>> handle: {:?}", handle);
    connection::process_ack(handle, &message)
        .await
        .map_err(to_napi_err)
}

#[napi]
pub fn connection_process_problem_report(handle: u32, problem_report: String) -> napi::Result<()> {
    trace!("connection_process_problem_report >>> handle: {:?}", handle);
    connection::process_problem_report(handle, &problem_report).map_err(to_napi_err)
}

#[napi]
pub async fn connection_send_response(handle: u32) -> napi::Result<()> {
    trace!("connection_send_response >>> handle: {:?}", handle);
    connection::send_response(handle).await.map_err(to_napi_err)
}

#[napi]
pub async fn connection_send_request(
    handle: u32,
    service_endpoint: String,
    routing_keys: Vec<String>,
) -> napi::Result<()> {
    trace!("connection_send_request >>> handle: {:?}", handle);
    connection::send_request(handle, service_endpoint, routing_keys)
        .await
        .map_err(to_napi_err)
}

#[napi]
pub async fn connection_send_ack(handle: u32) -> napi::Result<()> {
    trace!("connection_send_ack >>> handle: {:?}", handle);
    connection::send_ack(handle).await.map_err(to_napi_err)
}

#[napi]
pub async fn connection_send_generic_message(handle: u32, content: String) -> napi::Result<()> {
    trace!("connection_send_generic_message >>> handle: {:?}", handle);
    let id = Uuid::new_v4().to_string();
    let content = BasicMessageContent::builder()
        .content(content)
        .sent_time(Utc::now())
        .build();
    let decorators = BasicMessageDecorators::builder()
        .timing(Timing::builder().out_time(Utc::now()).build())
        .build();

    let message: BasicMessage = BasicMessage::builder()
        .id(id)
        .content(content)
        .decorators(decorators)
        .build();

    let message = AriesMessage::from(message);

    let basic_message = serde_json::to_string(&message)
        .map_err(From::from)
        .map_err(to_napi_err)?;

    connection::send_generic_message(handle, basic_message)
        .await
        .map_err(to_napi_err)
}

#[napi]
pub async fn connection_send_aries_message(handle: u32, content: String) -> napi::Result<()> {
    trace!("connection_send_generic_message >>> handle: {:?}", handle);
    connection::send_generic_message(handle, content)
        .await
        .map_err(to_napi_err)
}

#[napi]
pub async fn connection_create_invite(
    handle: u32,
    service_endpoint: String,
    routing_keys: Vec<String>,
) -> napi::Result<()> {
    trace!("connection_create_invite >>> handle: {:?}", handle);
    connection::create_invite(handle, service_endpoint, routing_keys)
        .await
        .map_err(to_napi_err)
}

#[napi]
pub fn connection_serialize(handle: u32) -> napi::Result<String> {
    trace!("connection_serialize >>> handle: {:?}", handle);
    connection::to_string(handle).map_err(to_napi_err)
}

#[napi]
pub fn connection_deserialize(connection_data: String) -> napi::Result<u32> {
    trace!("connection_deserialize >>>");
    connection::from_string(&connection_data).map_err(to_napi_err)
}

#[napi]
pub fn connection_release(handle: u32) -> napi::Result<()> {
    trace!("connection_release >>> handle: {:?}", handle);
    connection::release(handle).map_err(to_napi_err)
}
