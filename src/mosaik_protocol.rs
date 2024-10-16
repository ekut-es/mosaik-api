//! JSON serialization and deserialization for Mosaik messages. And handling of Mosaik requests.

use log::{debug, error, warn};
use serde::ser::{Serialize, SerializeTuple, Serializer};
use serde::{Deserialize, Deserializer};
use serde_json::{json, map::Map, Value};
use thiserror::Error;

use crate::types::{InputData, OutputRequest};
use crate::MosaikApi;

#[derive(Error, Debug)]
/// Collection of Error types to handle and differentiate errors in the Mosaik API.
///
/// # Variants
///
/// - `ParseError`: Error parsing a JSON request.
/// - `Serde`: Error during JSON serialization/deserialization.
/// - `UserError`: Container for user generated errors.
pub(crate) enum MosaikError {
    #[error("Parsing JSON Request: {0}")]
    ParseError(String),
    #[error("Serde JSON Error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("User generated Error: {0}")]
    UserError(String),
}

#[derive(Debug, PartialEq, Deserialize)]
/// Payload of the Network Message in the low-level Mosaik API.
pub(crate) struct MosaikMessage {
    /// [`MsgType`] is used to decide how to handle the message.
    msg_type: MsgType,
    /// unique [`MessageID`] for a message or message pair. Used to match a response to its request.
    id: MessageID,
    /// a JSON Value of arbitrary length.
    content: Value,
}

impl Serialize for MosaikMessage {
    /// Serialize a [`MosaikMessage`] to a tuple of 3 elements for the TCP connection.
    /// Should look like this: `[msg_type, id, content]`.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut tup = serializer.serialize_tuple(3)?;
        tup.serialize_element(&(self.msg_type as u8))?;
        tup.serialize_element(&self.id)?;
        tup.serialize_element(&self.content)?;
        tup.end()
    }
}

impl MosaikMessage {
    /// Serialize the [`MosaikMessage`] as a JSON byte vector
    ///
    /// # Errors
    /// If serialization fails, a fallback `ReplyFailure` message will be serialized instead.
    #[allow(clippy::expect_used)] // NOTE expect only used for the tested default messages which should not fail
    fn to_vec(&self) -> Vec<u8> {
        serde_json::to_vec(&self).unwrap_or_else(|e| {
            error!(
                "Failed to serialize a vector from the response to MessageID {}: {}",
                self.id, e
            );
            // Fallback to a `ReplyFailure` message for serialization error.
            // NOTE if this Message is changed, update [`test_serialize_to_vec_error_serializing_default()`].
            let error_response = MosaikMessage {
                msg_type: MsgType::ReplyFailure,
                id: self.id,
                // build an error response without e variable to ensure fixed size of MosaikMessage
                content: Value::from(
                    "Failed to serialize a vector from the response", // without Err(e) to maintain a small error msg size
                ),
            };
            serde_json::to_vec(&error_response)
                .expect("Tested error response should always be serializable.")
        })
    }

    /// Serialize a [`MosaikMessage`] to a vector of bytes for the TCP connection.
    /// The message is serialized to a vector of bytes with a 4-byte header.
    ///
    ///  # Format
    /// The resulting network message will follow this format:
    /// `\0x00\0x00\0x00\0x18[type, id, content]`
    ///
    /// # (No) Errors
    /// If serialization fails or the message exceeds `u32::MAX`, a fallback `ReplyFailure` message will be serialized instead.
    /// These messages are covered in this module's tests to ensure they are smaller than `u32::MAX` and always serialize.
    #[allow(clippy::expect_used)] // NOTE expect only used for the tested default messages which should not fail
    #[allow(clippy::cast_possible_truncation)]
    pub fn to_network_message(&self) -> Vec<u8> {
        // Serialize the content to a vector of bytes.
        let mut payload: Vec<u8> = self.to_vec();

        // Try to convert the payload length to `u32` for the header
        let header = u32::try_from(payload.len()).unwrap_or_else(|_| {
            error!(
                "MessageID {}: Message size exceeds allowed limit ({} bytes)",
                self.id,
                u32::MAX
            );
            // Fallback to a `ReplyFailure` message for message size error.
            // NOTE if this Message is changed, update [`test_serialize_to_vec_error_message_length_default()`].
            payload = serde_json::to_vec(&MosaikMessage {
                msg_type: MsgType::ReplyFailure,
                id: self.id,
                content: Value::from("Message too large"),
            })
            .expect("Tested error response should always be serializable.");
            // return the length of the ReplyFailure message, which is << u32::MAX
            payload.len() as u32
        });

        // Build the final message with a 4-byte header
        let mut message = header.to_be_bytes().to_vec();
        message.append(&mut payload);
        message
    }
}

#[repr(u8)]
#[derive(Debug, PartialEq, Copy, Clone)]
/// Matching integers used in the Mosaik API marking the three different Mosaik Message types.
enum MsgType {
    Request = 0,
    ReplySuccess = 1,
    ReplyFailure = 2,
}

/// Serialize [`MsgType`] as an unsigned integer.
impl Serialize for MsgType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8(*self as u8)
    }
}

/// Deserialize [`MsgType`] from an unsigned integer to the 3 valid variants.
impl<'de> Deserialize<'de> for MsgType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u8::deserialize(deserializer)?;
        match value {
            0 => Ok(MsgType::Request),
            1 => Ok(MsgType::ReplySuccess),
            2 => Ok(MsgType::ReplyFailure),
            _ => Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Unsigned(value.into()),
                &"expected a valid MsgType variant number. These are: 0, 1, 2",
            )),
        }
    }
}
type MessageID = u64;

#[derive(Debug, Deserialize, PartialEq)]
/// [`MsgType::Request`] type Message of the low-level Mosaik API.
/// Defines a `MosaikMessage` in a more granular way.
/// Dividing the [`MosaikMessage::content`] into method, args and kwargs.
///
/// See [`parse_json_request`] for the conversion from `MosaikMessage` to Request.
/// See [`handle_request`] for the Response creation to a Request.
pub(crate) struct Request {
    #[serde(skip)]
    msg_id: MessageID,
    method: String,
    args: Vec<Value>,
    kwargs: Map<String, Value>,
}

/// Serializing the request to the array needed for the `MosaikMessage` content.
impl Serialize for Request {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut tup = serializer.serialize_tuple(3)?;
        tup.serialize_element(&self.method)?;
        tup.serialize_element(&self.args)?;
        tup.serialize_element(&self.kwargs)?;
        tup.end()
    }
}

#[derive(Debug, PartialEq)]
/// This is for inner handling of a Response in the TCP Connection.
///
/// See [`MsgType`] for the two different Response types.
pub(crate) enum Response {
    /// signals the TCP Connection to reply with the [`MosaikMessage`]
    /// with the `msg_type` either a [`MsgType::ReplySuccess`] or [`MsgType::ReplyFailure`].
    Reply(MosaikMessage),
    /// signal to stop the Simulation and TCP Connection
    Stop,
}

/// Parse a JSON string to a `MosaikMessage` and check if it is a [Request].
pub(crate) fn parse_json_request(data: &str) -> Result<Request, MosaikError> {
    // Parse the string of data into serde_json::Value.
    let payload: MosaikMessage = match serde_json::from_str(data) {
        Ok(payload) => payload,
        Err(e) => {
            return Err(MosaikError::ParseError(format!(
                "Payload is not a valid Mosaik Message: {e}"
            )));
        }
    };

    if payload.msg_type != MsgType::Request {
        return Err(MosaikError::ParseError(format!(
            "The Mosaik Message is not a request: {payload:?}"
        )));
    }

    let mut request: Request = match serde_json::from_value(payload.content) {
        Ok(request) => request,
        Err(e) => {
            return Err(MosaikError::ParseError(format!(
                "The Mosaik Message has no valid Request content: {e}"
            )));
        }
    };
    request.msg_id = payload.id;
    Ok(request)
}

/// Handle a [Request] and return a [Response].
/// See [`MsgType`] for the two different Response types.
/// Uses several helper functions to handle the different Mosaik API calls.
pub(crate) fn handle_request<T: MosaikApi>(simulator: &mut T, request: Request) -> Response {
    let id = request.msg_id;
    let handle_result = match request.method.as_ref() {
        "init" => handle_init(simulator, request),
        "create" => handle_create(simulator, request),
        "step" => handle_step(simulator, request),
        "get_data" => handle_get_data(simulator, request),
        "setup_done" => handle_setup_done(simulator),
        "stop" => {
            debug!("Received stop request");
            simulator.stop();
            return Response::Stop;
        }
        method => simulator
            .extra_method(method, &request.args, &request.kwargs)
            .map_err(MosaikError::UserError),
    };

    match handle_result {
        Ok(content) => Response::Reply(MosaikMessage {
            msg_type: MsgType::ReplySuccess,
            id,
            content,
        }),
        Err(mosaik_error) => Response::Reply(MosaikMessage {
            msg_type: MsgType::ReplyFailure,
            id,
            content: json!(mosaik_error.to_string()),
        }),
    }
}

/// Helper function to handle the `init` API call. See [`MosaikApi::init`].
///
/// # Example
/// ["init", \[`sim_id`\], {`time_resolution=time_resolution`, **`sim_params`}] -> meta
fn handle_init<T: MosaikApi>(simulator: &mut T, request: Request) -> Result<Value, MosaikError> {
    let mut request = request;
    let sid = request
        .args
        .first()
        .ok_or_else(|| MosaikError::ParseError("Missing SimId in init request".to_string()))?
        .as_str()
        .ok_or_else(|| MosaikError::ParseError("Invalid SimId format".to_string()))?
        .to_string();

    let time_resolution = request
        .kwargs
        .remove("time_resolution")
        .and_then(|v| v.as_f64())
        .unwrap_or_else(|| {
            warn!("Invalid time resolution provided, defaulting to 1.0");
            1.0f64
        }); // TODO check if default is applicable or if it should be an error

    let sim_params = request.kwargs;

    match simulator.init(sid, time_resolution, sim_params) {
        Ok(meta) => Ok(serde_json::to_value(meta)?),
        Err(e) => Err(MosaikError::UserError(e)),
    }
}

/// Helper function to handle the `create` API call. See [`MosaikApi::create`].
///
/// # Example
/// ["create", [num, model], {**`model_params`}] -> `entity_list`
fn handle_create<T: MosaikApi>(simulator: &mut T, request: Request) -> Result<Value, MosaikError> {
    let num: usize = request
        .args
        .first()
        .ok_or_else(|| {
            MosaikError::ParseError("Missing number of instances in create request".to_string())
        })?
        .as_u64() // use as_u64() to get number
        .ok_or_else(|| MosaikError::ParseError("Invalid number format".to_string()))?
        .try_into() // cast safely to usize
        .map_err(|_| {
            MosaikError::ParseError(format!(
                "Num in create request is too large. Maximum is {:?}",
                usize::MAX
            ))
        })?;
    let model_name: String = request
        .args
        .get(1)
        .ok_or_else(|| MosaikError::ParseError("Missing model_name in create request".to_string()))?
        .as_str()
        .ok_or_else(|| MosaikError::ParseError("Invalid model_name format".to_string()))?
        .to_string();

    let kwargs = request.kwargs;

    match simulator.create(num, model_name, kwargs) {
        Ok(create_result) => Ok(serde_json::to_value(create_result)?),
        Err(e) => Err(MosaikError::UserError(e)),
    }
}

/// Helper function to handle the `step` API call. See [`MosaikApi::step`].
///
/// # Example
/// ["step", [time, inputs, `max_advance`], {}] -> Optional\[`time_next_step`\]
fn handle_step<T: MosaikApi>(simulator: &mut T, request: Request) -> Result<Value, MosaikError> {
    let time: u64 = request
        .args
        .first()
        .ok_or_else(|| MosaikError::ParseError("Missing time in step request".to_string()))?
        .as_u64()
        .ok_or_else(|| MosaikError::ParseError("Invalid time format".to_string()))?;

    let inputs: InputData = serde_json::from_value(
        request
            .args
            .get(1)
            .ok_or_else(|| MosaikError::ParseError("Missing inputs in step request".to_string()))?
            .to_owned(),
    )
    .map_err(|e| {
        MosaikError::ParseError(format!("Failed to parse inputs from step request: {e}"))
    })?;

    let max_advance: u64 = request
        .args
        .get(2)
        .ok_or_else(|| MosaikError::ParseError("Missing max_advance in step request".to_string()))?
        .as_u64()
        .ok_or_else(|| MosaikError::ParseError("Invalid max_advance format".to_string()))?;

    match simulator.step(time, inputs, max_advance) {
        Ok(value) => Ok(serde_json::to_value(value)?),
        Err(e) => Err(MosaikError::UserError(e)),
    }
}

/// Helper function to handle the `get_data` API call. See [`MosaikApi::get_data`].
///
/// # Example
/// \["`get_data`", \[outputs\], {}\] -> data
fn handle_get_data<T: MosaikApi>(
    simulator: &mut T,
    request: Request,
) -> Result<Value, MosaikError> {
    let outputs: OutputRequest = serde_json::from_value(
        request
            .args
            .first()
            .ok_or_else(|| {
                MosaikError::ParseError("Missing outputs in get_data request".to_string())
            })?
            .to_owned(),
    )
    .map_err(|e| {
        MosaikError::ParseError(format!(
            "Failed to parse outputs from get_data request: {e}"
        ))
    })?;

    match simulator.get_data(outputs) {
        Ok(output_data) => Ok(serde_json::to_value(output_data)?),
        Err(e) => Err(MosaikError::UserError(e)),
    }
}

/// Helper function to handle the `setup_done` API call. See [`MosaikApi::setup_done`].
///
/// # Example
/// ["`setup_done`", [], {}] -> null
fn handle_setup_done<T: MosaikApi>(simulator: &mut T) -> Result<Value, MosaikError> {
    match simulator.setup_done() {
        Ok(()) => Ok(json!(null)),
        Err(e) => Err(MosaikError::UserError(e)),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::types::{CreateResult, InputData, Meta, OutputData, OutputRequest, SimulatorType};
    use crate::MockMosaikApi;

    use mockall::predicate::*;
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::LazyLock;

    // --------------------------------------------------------------------------
    // Tests for MosaikMessage
    // --------------------------------------------------------------------------

    #[test]
    fn test_serialize_mosaik_message() {
        let expect = r#"[0,123,["my_func",["hello","world"],{"times":23}]]"#
            .as_bytes()
            .to_vec();
        let request = Request {
            msg_id: 123,
            method: "my_func".to_string(),
            args: vec![json!("hello"), json!("world")],
            kwargs: {
                let mut map = Map::new();
                map.insert("times".to_string(), json!(23));
                map
            },
        };
        let actual = MosaikMessage {
            msg_type: MsgType::Request,
            id: request.msg_id,
            content: json!(request),
        }
        .to_network_message();
        assert_eq!(&actual[4..], expect);
    }

    #[test]
    fn test_serialize_response_success_to_vec() {
        let expect = r#"[1,1,"the return value"]"#.as_bytes().to_vec();
        let actual = MosaikMessage {
            msg_type: MsgType::ReplySuccess,
            id: 1,
            content: json!("the return value"),
        }
        .to_network_message();

        // NOTE mosaik tutorial is wrong and has 2 bytes too many (should be 24B)
        assert_eq!(actual[0..4], vec![0x00, 0x00, 0x00, 0x18]);
        assert_eq!(actual.len(), 4 + 0x18);
        assert_eq!(&actual[4..], expect);
    }

    #[test]
    fn test_serialize_response_failure_to_vec() {
        let expect = r#"[2,1,"Error in your code line 23: ..."]"#.as_bytes().to_vec();
        let actual = MosaikMessage {
            msg_type: MsgType::ReplyFailure,
            id: 1,
            content: json!("Error in your code line 23: ..."),
        }
        .to_network_message();

        // NOTE mosaik Tutorial has 2 bytes too many (should be 39B)
        assert_eq!(actual[..4], vec![0x00, 0x00, 0x00, 0x27]);
        assert_eq!(actual.len(), 4 + 0x27);
        assert_eq!(actual[4..], expect);
    }

    #[test]
    fn test_serialize_response_error_to_vec() {
        let expect = r#"[2,123,"User generated Error: some error"]"#.as_bytes().to_vec();
        let error = MosaikError::UserError("some error".to_string());
        let actual = MosaikMessage {
            msg_type: MsgType::ReplyFailure,
            id: 123,
            content: json!(error.to_string()),
        }
        .to_network_message();
        assert_eq!(actual.len(), 4 + expect.len());
        assert_eq!(actual[4..], expect);
    }

    #[test]
    fn test_serialize_to_vec_error_serializing_default() {
        let expect = r#"[2,18446744073709551615,"Failed to serialize a vector from the response"]"#
            .as_bytes()
            .to_vec();
        let error_response = MosaikMessage {
            msg_type: MsgType::ReplyFailure,
            id: u64::MAX,
            // build an error response without e variable to ensure fixed size of MosaikMessage
            content: Value::from(
                "Failed to serialize a vector from the response", // without Err(e) to maintain a small error msg size
            ),
        };
        assert!(expect.len() < u32::MAX as usize);
        let to_vec = serde_json::to_vec(&error_response);
        assert!(&to_vec.is_ok(), "Default response should be serializable.");
        assert_eq!(
            to_vec.unwrap(),
            expect,
            "JSON Serialized response should equal the expected response."
        );
        let actual = error_response.to_network_message();
        assert_eq!(actual.len(), 4 + expect.len());
        assert_eq!(actual[4..], expect);
    }

    #[test]
    fn test_serialize_to_vec_error_message_length_default() {
        let expect = r#"[2,18446744073709551615,"Message too large"]"#.as_bytes().to_vec();
        let error_response = MosaikMessage {
            msg_type: MsgType::ReplyFailure,
            id: u64::MAX,
            content: Value::from("Message too large"),
        };
        assert!(expect.len() < u32::MAX as usize);
        let to_vec = serde_json::to_vec(&error_response);
        assert!(&to_vec.is_ok(), "Default response should be serializable.");
        assert_eq!(
            to_vec.unwrap(),
            expect,
            "JSON Serialized response should equal the expected response."
        );
        let actual = error_response.to_network_message();
        assert_eq!(actual.len(), 4 + expect.len());
        assert_eq!(actual[4..], expect);
    }

    // --------------------------------------------------------------------------
    // Tests for MsgType
    // --------------------------------------------------------------------------

    #[test]
    fn test_msg_type_deserialization() {
        let actual: MsgType = serde_json::from_str("0").unwrap();
        assert_eq!(actual, MsgType::Request);

        let actual: MsgType = serde_json::from_str("1").unwrap();
        assert_eq!(actual, MsgType::ReplySuccess);

        let actual: MsgType = serde_json::from_str("2").unwrap();
        assert_eq!(actual, MsgType::ReplyFailure);
    }

    #[test]
    fn test_msg_type_deserialization_error() {
        let actual: Result<MsgType, serde_json::Error> = serde_json::from_str("3");
        assert!(actual.is_err());
        assert!(actual.unwrap_err().is_data());
    }

    #[test]
    fn test_msg_type_serialization() {
        let actual = json!(&MsgType::Request);
        assert_eq!(actual, json!(0));
        let actual = json!(&MsgType::ReplySuccess);
        assert_eq!(actual, json!(1));
        let actual = json!(&MsgType::ReplyFailure);
        assert_eq!(actual, json!(2));
    }

    // -------------------------------------------------------------------------
    // Tests for `parse_json_request`
    // -------------------------------------------------------------------------

    #[test]
    fn test_parse_valid_request() {
        let valid_request = r#"[0, 1, ["my_func", ["hello", "world"], {"times": 23}]]"#;
        let expected = Request {
            msg_id: 1,
            method: "my_func".to_string(),
            args: vec![json!("hello"), json!("world")],
            kwargs: {
                let mut map = Map::new();
                map.insert("times".to_string(), json!(23));
                map
            },
        };
        let result = parse_json_request(valid_request);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_parse_invalid_mosaik_message() {
        let data = r"invalid request format";
        let result = parse_json_request(data);
        assert!(result.is_err());
        let expect = MosaikError::ParseError("Payload is not a valid Mosaik Message:".to_string());
        let actual = result.unwrap_err();
        assert!(
            actual.to_string().starts_with(&expect.to_string()),
            "{actual} does not start with {expect}"
        );
    }

    #[test]
    fn test_parse_success_reply() {
        let data = r#"[1, 1, "return value"]"#;
        let result = parse_json_request(data);
        assert!(result.is_err());
        let expect = MosaikError::ParseError("The Mosaik Message is not a request:".to_string());
        let actual = result.unwrap_err();
        assert!(
            actual.to_string().starts_with(&expect.to_string()),
            "{actual} does not start with {expect}"
        );
    }

    #[test]
    fn test_parse_failure_reply() {
        let data = r#"[2, 1, "Error in your code line 23: ..."]"#;
        let result = parse_json_request(data);
        assert!(result.is_err());
        let expect = MosaikError::ParseError("The Mosaik Message is not a request:".to_string());
        let actual = result.unwrap_err();
        assert!(
            actual.to_string().starts_with(&expect.to_string()),
            "{actual} does not start with {expect}"
        );
    }

    #[test]
    fn test_parse_invalid_message_type() {
        let data = r#"["0", 1, ["my_func", ["hello", "world"], {"times": 23}]]"#;
        let result = parse_json_request(data);
        assert!(result.is_err());
        let expect = MosaikError::ParseError("Payload is not a valid Mosaik Message:".to_string());
        let actual = result.unwrap_err();
        assert!(
            actual.to_string().starts_with(&expect.to_string()),
            "{actual} does not start with {expect}"
        );
    }

    #[test]
    fn test_parse_invalid_request_format() {
        let data = r#"[0, 1, ["my_func", {"hello": "world"}, {"times": 23}]]"#;
        let result = parse_json_request(data);
        assert!(result.is_err());
        let expect =
            MosaikError::ParseError("The Mosaik Message has no valid Request content:".to_string());
        let actual = result.unwrap_err();
        assert!(
            actual.to_string().starts_with(&expect.to_string()),
            "{actual} does not start with {expect}"
        );
    }

    #[test]
    fn test_parse_step_request() {
        let valid_step_request = r#"
        [0, 1, ["step",
                [1,
                 {"eid_1": {"attr_1": {"src_full_id_1": 2, "src_full_id_2": 4},
                            "attr_2": {"src_full_id_1": 3, "src_full_id_2": 5}
                            }
                },
                200
                ], {}
              ]
        ]"#;

        let expected = Request {
            msg_id: 1,
            method: "step".to_string(),
            args: vec![
                json!(1),
                json!({"eid_1": {"attr_1": {"src_full_id_1": 2, "src_full_id_2": 4}, "attr_2": {"src_full_id_1": 3, "src_full_id_2": 5}}}),
                json!(200),
            ],
            kwargs: Map::new(),
        };
        let result = parse_json_request(valid_step_request);
        assert!(result.is_ok());

        let request = result.unwrap();
        let input: InputData = serde_json::from_value(request.args[1].clone()).unwrap();

        assert_eq!(
            input
                .get("eid_1")
                .unwrap()
                .get("attr_2")
                .unwrap()
                .get("src_full_id_1")
                .unwrap(),
            3
        );
        assert_eq!(request, expected);
    }

    #[test]
    fn test_parse_get_data_request() {
        let valid_request = r#"[0, 1, ["get_data", [{"eid_1": ["attr_1", "attr_2"]}], {}]]"#;
        let mut outputs = Map::new();
        outputs.insert("eid_1".to_string(), json!(vec!["attr_1", "attr_2"]));
        let expected = Request {
            msg_id: 1,
            method: "get_data".to_string(),
            args: vec![json!(outputs)],
            kwargs: Map::new(),
        };
        assert!(parse_json_request(valid_request).is_ok());
        assert_eq!(parse_json_request(valid_request).unwrap(), expected);
    }

    // ------------------------------------------------------------------------
    // Tests for `Response serialize`
    // ------------------------------------------------------------------------

    #[test]
    fn test_serialize_request() {
        let expect = r#"["my_func",["hello","world"],{"times":23}]"#;
        let request = Request {
            msg_id: 123,
            method: "my_func".to_string(),
            args: vec![json!("hello"), json!("world")],
            kwargs: {
                let mut map = Map::new();
                map.insert("times".to_string(), json!(23));
                map
            },
        };
        let ser_request = json!(request);
        assert_eq!(ser_request.to_string(), expect);
    }

    // ------------------------------------------------------------------------
    // Tests for `handle_request`
    // ------------------------------------------------------------------------

    //     Request:

    // ["init", ["PowerGridSim-0"], {"step_size": 60}]

    // Reply:

    // {
    //    "api_version": "2.2",
    //    "models": {
    //         "Grid": {
    //             "public": true,
    //             "params": ["topology_file"],
    //             "attrs": []
    //         },
    //         "Node": {
    //             "public": false,
    //             "params": [],
    //             "attrs": ["P", "Q"]
    //         },
    //         "Branch": {
    //             "public": false,
    //             "params": [],
    //             "attrs": ["I", "I_max"]
    //         }
    //     }
    // }
    #[test]
    fn test_handle_request_init_success() {
        static META: LazyLock<Meta> =
            LazyLock::new(|| Meta::new(SimulatorType::default(), HashMap::new(), None));
        let mut simulator = MockMosaikApi::new();
        let request = Request {
            msg_id: 789,
            method: "init".to_string(),
            args: vec![json!("simID-1")],
            kwargs: Map::new(), // no other params
        };

        simulator
            .expect_init()
            .once()
            .with(eq("simID-1".to_string()), eq(1.0), eq(Map::new()))
            .returning(|_, _, _| Ok(&META));

        let payload = json!(Meta::new(SimulatorType::default(), HashMap::new(), None));
        let actual_response = handle_request(&mut simulator, request);
        assert_eq!(
            actual_response,
            Response::Reply(MosaikMessage {
                msg_type: MsgType::ReplySuccess,
                id: 789,
                content: payload
            })
        );
    }

    #[test]
    fn test_handle_request_init_failure() {
        let mut kwargs = Map::new();
        kwargs.insert("time_resolution".to_string(), json!(0.1));
        kwargs.insert("step_size".to_string(), json!(60));
        let request = Request {
            msg_id: 123,
            method: "init".to_string(),
            args: vec![json!(0)], // invalid SimId
            kwargs: kwargs.clone(),
        };

        let mut mock_simulator = MockMosaikApi::new();
        mock_simulator.expect_init().never();

        let actual = handle_request(&mut mock_simulator, request);
        let expected = Response::Reply(MosaikMessage {
            msg_type: MsgType::ReplyFailure,
            id: 123,
            content: json!(MosaikError::ParseError("Invalid SimId format".to_string()).to_string()),
        });
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_handle_request_init_user_error() {
        let mut simulator = MockMosaikApi::new();
        let request = Request {
            msg_id: 789,
            method: "init".to_string(),
            args: vec![json!("arg1"), json!("arg2")],
            kwargs: Map::new(),
        };

        simulator
            .expect_init()
            .with(eq("arg1".to_string()), eq(1.0), eq(Map::new()))
            .returning(|_, _, _| Err("some custom error".to_string()));

        let expected_response = Response::Reply(MosaikMessage {
            msg_type: MsgType::ReplyFailure,
            id: 789,
            content: json!(MosaikError::UserError("some custom error".to_string()).to_string()),
        });
        let actual_response = handle_request(&mut simulator, request);

        assert_eq!(actual_response, expected_response);
    }

    // ------------------------------------------------------------------------

    // Request:

    // ["create", [1, "Grid"], {"topology_file": "data/grid.json"}]

    // Reply:

    // [
    //     {
    //         "eid": "Grid_1",
    //         "type": "Grid",
    //         "rel": [],
    //         "children": [
    //             {
    //                 "eid": "node_0",
    //                 "type": "Node",
    //             },
    //             {
    //                 "eid": "node_1",
    //                 "type": "Node",
    //             },
    //             {
    //                 "eid": "branch_0",
    //                 "type": "Branch",
    //                 "rel": ["node_0", "node_1"]
    //             }
    //         ]
    //     }
    // ]

    #[test]
    fn test_handle_request_create() {
        let request = Request {
            msg_id: 1,
            method: "create".to_string(),
            args: vec![json!(1), json!("Grid")],
            kwargs: {
                let mut map = Map::new();
                map.insert("topology_file".to_string(), "data/grid.json".into());
                map
            },
        };

        let mut cr = CreateResult::new("Grid_1".to_string(), "Grid".to_string());
        cr.children = Some(vec![
            CreateResult::new("node_0".to_string(), "Node".to_string()),
            CreateResult::new("node_1".to_string(), "Node".to_string()),
            CreateResult {
                eid: "branch_0".to_string(),
                model_type: "Branch".to_string(),
                rel: Some(vec!["node_0".to_string(), "node_1".to_string()]),
                children: None,
                extra_info: None,
            },
        ]);
        let expect = MosaikMessage {
            msg_type: MsgType::ReplySuccess,
            id: request.msg_id,
            content: serde_json::to_value(vec![cr.clone()]).unwrap(),
        };
        let mut mock_simulator = MockMosaikApi::new();
        mock_simulator
            .expect_create()
            .once()
            .with(eq(1), eq("Grid".to_string()), eq(request.kwargs.clone()))
            .returning(move |_, _, _| Ok(vec![cr.clone()]));

        let result = handle_request(&mut mock_simulator, request);
        assert_eq!(result, Response::Reply(expect));
    }

    // ------------------------------------------------------------------------

    // Request:

    // ["setup_done", [], {}]

    // Reply:

    // null

    #[test]
    fn test_handle_request_setup_done() {
        let request = Request {
            msg_id: 1,
            method: "setup_done".to_string(),
            args: vec![],
            kwargs: Map::new(),
        };

        let expect = MosaikMessage {
            msg_type: MsgType::ReplySuccess,
            id: request.msg_id,
            content: serde_json::Value::Null,
        };
        let mut mock_simulator = MockMosaikApi::new();
        mock_simulator
            .expect_setup_done()
            .once()
            .with()
            .returning(move || Ok(()));

        let result = handle_request(&mut mock_simulator, request);
        assert_eq!(result, Response::Reply(expect));
    }

    // ------------------------------------------------------------------------

    // NOTE this mosaik example from the official documentation is wrong formatting
    // -- example definition in the paragraph above is correct
    // test was fixed using InputData (see types.rs) format instead of Array

    // Request:

    // [
    //     "step",
    //     [
    //         60,
    //         {
    //               "node_1": {"P": [20, 3.14], "Q": [3, -2.5]},
    //               "node_2": {"P": [42], "Q": [-23.2]},
    //         },
    //         3600
    //     ],
    //     {}
    // ]

    // Reply:

    // 120

    #[test]
    fn test_handle_request_step() {
        let request = Request {
            msg_id: 1,
            method: "step".to_string(),
            args: vec![
                json!(60),
                json!({"node_1": {"P": {"full_id1":20, "full_id2":3.1}, "Q": {"full_id1":3,"full_id2": -2.5}},
                       "node_2": {"P": {"full_id1":42}, "Q": {"full_id1":-23.2}}}),
                json!(3600),
            ],
            kwargs: Map::new(),
        };

        let expect = MosaikMessage {
            msg_type: MsgType::ReplySuccess,
            id: request.msg_id,
            content: json!(120),
        };
        let mut mock_simulator = MockMosaikApi::new();
        mock_simulator
            .expect_step()
            .once()
            .with(
                eq(60),
                eq(serde_json::from_value::<InputData>(request.args[1].clone()).unwrap()),
                eq(3600),
            )
            .returning(move |_, _, _| Ok(Some(120)));

        let result = handle_request(&mut mock_simulator, request);
        assert_eq!(result, Response::Reply(expect));
    }

    // ------------------------------------------------------------------------

    // Request:

    // ["get_data", [{"branch_0": ["I"]}], {}]

    // Reply:

    // {
    //     "branch_0": {
    //         "I": 42.5
    //     }
    // }

    #[test]
    fn test_handle_request_get_data() {
        let request = Request {
            msg_id: 1,
            method: "get_data".to_string(),
            args: vec![json!({"branch_0": ["I"]})],
            kwargs: Map::new(),
        };

        let expect = MosaikMessage {
            msg_type: MsgType::ReplySuccess,
            id: request.msg_id,
            content: json!({"branch_0": {"I": 42.5}, "time": 123}),
        };
        let mut mock_simulator = MockMosaikApi::new();
        mock_simulator
            .expect_get_data()
            .once()
            .with(eq(serde_json::from_value::<OutputRequest>(
                request.args[0].clone(),
            )
            .unwrap()))
            .returning(move |_| {
                Ok(serde_json::from_value::<OutputData>(
                    json!({"branch_0": {"I": 42.5}, "time": 123}),
                )
                .unwrap())
            });

        let result = handle_request(&mut mock_simulator, request);
        assert_eq!(result, Response::Reply(expect));
    }

    // ------------------------------------------------------------------------

    // Request:

    // ["stop", [], {}]

    // Reply:

    //     no reply required

    // expecting Stop signal for tcp

    #[test]
    fn test_handle_request_stop() {
        let request = Request {
            msg_id: 1,
            method: "stop".to_string(),
            args: vec![],
            kwargs: Map::new(),
        };

        let mut mock_simulator = MockMosaikApi::new();
        mock_simulator
            .expect_stop()
            .once()
            .with()
            .returning(move || ());

        let result = handle_request(&mut mock_simulator, request);
        assert_eq!(result, Response::Stop);
    }

    // ------------------------------------------------------------------------

    // Asynchronous requests
    //     Request:

    // ["get_progress", [], {}]

    // Reply:

    // 23.42

    //     Request:

    // ["get_related_entities", [["grid_sim_0.node_0", "grid_sim_0.node_1"]] {}]

    // Reply:

    // {
    //     "grid_sim_0.node_0": {
    //         "grid_sim_0.branch_0": {"type": "Branch"},
    //         "pv_sim_0.pv_0": {"type": "PV"}
    //     },
    //     "grid_sim_0.node_1": {
    //         "grid_sim_0.branch_0": {"type": "Branch"}
    //     }
    // }

    // DEPRECATED!
    // Request:

    // ["get_data", [{"grid_sim_0.branch_0": ["I"]}], {}]

    // Reply:

    // {
    //     "grid_sim_0.branch_0": {
    //         "I": 42.5
    //     }
    // }

    // DEPRECATED!
    // Request:

    // [
    //     "set_data",
    //     [{
    //         "mas_0.agent_0": {"pvsim_0.pv_0": {"P_target": 20,
    //                                            "Q_target": 3.14}},
    //         "mas_0.agent_1": {"pvsim_0.pv_1": {"P_target": 21,
    //                                            "Q_target": 2.718}}
    //     }],
    //     {}
    // ]

    // Reply:

    // null
}
