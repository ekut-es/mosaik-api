use log::*;
use serde::ser::{Serialize, SerializeTuple, Serializer};
use serde::Deserialize;
use serde_json::{json, map::Map, to_vec, Value};

use thiserror::Error;

use crate::{MosaikApi, SimId};

#[derive(Error, Debug)]
pub enum MosaikError {
    // TODO separate error for handle_* functions?
    #[error("Parsing JSON Request: {0}")]
    ParseError(String),
    #[error("Serde JSON Error: {0}")]
    Serde(#[from] serde_json::Error),
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct MosaikMessage {
    pub msg_type: u8,
    pub id: MessageID,
    pub content: Value,
}

impl MosaikMessage {
    pub fn serialize_to_vec(&self) -> Vec<u8> {
        let response: Value = json!([self.msg_type, self.id, self.content]);
        match to_vec(&response) {
            Ok(vec) => {
                let mut header = (vec.len() as u32).to_be_bytes().to_vec();
                header.append(&mut vec.clone());
                header
            }
            Err(e) => {
                // return a FailureReply with the error message
                error!(
                    "Failed to serialize response to MessageID {}: {}",
                    self.id, e
                );
                let error_message = format!(
                    "Failed to serialize a vector from the response to MessageID {}",
                    self.id
                );
                let error_response = json!([MSG_TYPE_REPLY_FAILURE, self.id, error_message]);
                to_vec(&error_response).unwrap() // FIXME unwrap should be safe, because we know the error message is a short enough string
            }
        }
    }
}

// TODO can we use this as an enum for msg_type without casting it to u8 all the time? Deserialization of MosaikMessage would get difficult. See test_deserialize_u8_to_enum
pub const MSG_TYPE_REQUEST: u8 = 0;
pub const MSG_TYPE_REPLY_SUCCESS: u8 = 1;
pub const MSG_TYPE_REPLY_FAILURE: u8 = 2;
// #[repr(u8)]
// #[derive(Debug, PartialEq, Copy, Clone, Deserialize)]  // NOTE one could use this to check validity via MsgType::try_from(msg_type)
// pub enum MsgType {
//     Request = 0,
//     ReplySuccess = 1,
//     ReplyFailure = 2,
// }

type MessageID = u64;

#[derive(Debug, Deserialize, PartialEq)]
pub struct Request {
    #[serde(skip)]
    msg_id: MessageID,
    method: String,
    args: Vec<Value>,
    kwargs: Map<String, Value>,
}

impl Serialize for Request {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serializing the request to the array needed for the MosaikMessage content
        let mut tup = serializer.serialize_tuple(3)?;
        tup.serialize_element(&self.method)?;
        tup.serialize_element(&self.args)?;
        tup.serialize_element(&self.kwargs)?;
        tup.end()
    }
}

#[derive(Debug, PartialEq)]
pub enum Response {
    Reply(MosaikMessage),
    Stop,
    NoReply,
}

pub fn parse_json_request(data: &str) -> Result<Request, MosaikError> {
    // Parse the string of data into serde_json::Value.
    let payload: MosaikMessage = match serde_json::from_str(data) {
        Ok(payload) => payload,
        Err(e) => {
            return Err(MosaikError::ParseError(format!(
                "Payload is not a valid Mosaik Message: {}",
                e
            )));
        }
    };

    if payload.msg_type != MSG_TYPE_REQUEST {
        return Err(MosaikError::ParseError(format!(
            "The Mosaik Message is not a request: {:?}",
            payload
        )));
    }

    let mut request: Request = match serde_json::from_value(payload.content) {
        Ok(request) => request,
        Err(e) => {
            return Err(MosaikError::ParseError(format!(
                "The Mosaik Message has no valid Request content: {}",
                e
            )));
        }
    };
    request.msg_id = payload.id;
    Ok(request)
}

pub fn handle_request<T: MosaikApi>(simulator: &mut T, request: &Request) -> Response {
    let handle_result = match request.method.as_ref() {
        "init" => handle_init(simulator, request),
        "create" => handle_create(simulator, request),
        "step" => handle_step(simulator, request),
        "get_data" => handle_get_data(simulator, request),
        "setup_done" => {
            simulator.setup_done();
            Ok(Value::Null)
        }
        "stop" => {
            debug!("Received stop command!");
            simulator.stop();
            return Response::Stop;
        }
        method => {
            error!(
                "Unimplemented method {:?} requested. Simulation should most likely stop now",
                method
            );
            return Response::Reply(MosaikMessage {
                msg_type: MSG_TYPE_REPLY_FAILURE,
                id: request.msg_id,
                content: json!(format!(
                    "Unimplemented method {:?} requested. Simulation should most likely stop now",
                    method
                )),
            });
        }
    };

    match handle_result {
        Ok(content) => Response::Reply(MosaikMessage {
            msg_type: MSG_TYPE_REPLY_SUCCESS,
            id: request.msg_id,
            content,
        }),
        Err(mosaik_error) => Response::Reply(MosaikMessage {
            msg_type: MSG_TYPE_REPLY_FAILURE,
            id: request.msg_id,
            content: json!(mosaik_error.to_string()),
        }),
    }
}

fn handle_init<T: MosaikApi>(simulator: &mut T, request: &Request) -> Result<Value, MosaikError> {
    /* TODO do we want this?
    let sid: SimId = match serde_json::from_value(request.args[0].clone()) {
        Ok(sid) => sid,
        Err(e) => {
            return Err(MosaikError::ParseError(format!(
                "Could not parse Simulator ID from Mosaik Message args: {}",
                e
            )))
        }
    };*/

    Ok(serde_json::to_value(
        simulator.init(
            serde_json::from_value(request.args[0].clone())?, // sid,
            request
                .kwargs
                .get("time_resolution")
                .and_then(|value| value.as_f64())
                .unwrap_or(1.0f64),
            request.kwargs.clone(),
        ),
    )?)
}

fn handle_create<T: MosaikApi>(simulator: &mut T, request: &Request) -> Result<Value, MosaikError> {
    Ok(serde_json::to_value(simulator.create(
        serde_json::from_value(request.args[0].clone())?,
        serde_json::from_value(request.args[1].clone())?,
        request.kwargs.clone(),
    ))?)
}

fn handle_step<T: MosaikApi>(simulator: &mut T, request: &Request) -> Result<Value, MosaikError> {
    Ok(Value::from(simulator.step(
        serde_json::from_value(request.args[0].clone())?,
        serde_json::from_value(request.args[1].clone())?,
        serde_json::from_value(request.args[2].clone())?,
    ))) // add handling of optional return
}

fn handle_get_data<T: MosaikApi>(
    simulator: &mut T,
    request: &Request,
) -> Result<Value, MosaikError> {
    Ok(serde_json::to_value(simulator.get_data(
        serde_json::from_value(request.args[0].clone())?,
    ))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{InputData, Meta, SimulatorType};
    use crate::{CreateResult, MockMosaikApi, OutputData, OutputRequest};

    use mockall::predicate::*;
    use serde_json::{json, to_vec, Value};
    use std::{
        any::{Any, TypeId},
        collections::HashMap,
    };

    // -------------------------------------------------------------------------
    // Tests for `parse_json_request`
    // -------------------------------------------------------------------------

    #[test]
    fn test_parse_valid_request() -> Result<(), MosaikError> {
        let valid_request = r#"[0, 1, ["my_func", ["hello", "world"], {"times": 23}]]"#;
        let expected = Request {
            msg_id: 1,
            method: "my_func".to_string(),
            args: vec![json!("hello"), json!("world")],
            kwargs: {
                let mut map = Map::new();
                map.insert("times".to_string(), json!(23))
                    .unwrap_or_default();
                map
            },
        };
        let result = parse_json_request(valid_request);
        assert!(result.is_ok());
        assert_eq!(result?, expected);

        Ok(())
    }

    #[test]
    fn test_parse_invalid_mosaik_message() {
        let data = r#"invalid request format"#;
        let result = parse_json_request(data);
        assert!(result.is_err());
        let expect = MosaikError::ParseError("Payload is not a valid Mosaik Message:".to_string());
        let actual = result.unwrap_err();
        assert_eq!(
            actual.to_string().starts_with(&expect.to_string()),
            true,
            "{} does not start with {}",
            actual,
            expect
        );
    }

    #[test]
    fn test_parse_success_reply() {
        let data = r#"[1, 1, "return value"]"#;
        let result = parse_json_request(data);
        assert!(result.is_err());
        let expect = MosaikError::ParseError("The Mosaik Message is not a request:".to_string());
        let actual = result.unwrap_err();
        assert_eq!(
            actual.to_string().starts_with(&expect.to_string()),
            true,
            "{} does not start with {}",
            actual,
            expect
        );
    }

    #[test]
    fn test_parse_failure_reply() {
        let data = r#"[2, 1, "Error in your code line 23: ..."]"#;
        let result = parse_json_request(data);
        assert!(result.is_err());
        let expect = MosaikError::ParseError("The Mosaik Message is not a request:".to_string());
        let actual = result.unwrap_err();
        assert_eq!(
            actual.to_string().starts_with(&expect.to_string()),
            true,
            "{} does not start with {}",
            actual,
            expect
        );
    }

    #[test]
    fn test_parse_invalid_message_type() {
        let data = r#"["0", 1, ["my_func", ["hello", "world"], {"times": 23}]]"#;
        let result = parse_json_request(data);
        assert!(result.is_err());
        let expect = MosaikError::ParseError("Payload is not a valid Mosaik Message:".to_string());
        let actual = result.unwrap_err();
        assert_eq!(
            actual.to_string().starts_with(&expect.to_string()),
            true,
            "{} does not start with {}",
            actual,
            expect
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
        assert_eq!(
            actual.to_string().starts_with(&expect.to_string()),
            true,
            "{} does not start with {}",
            actual,
            expect
        );
    }
    // TODO maybe the next two tests are redundant
    #[test]
    fn test_parse_step_request() -> Result<(), MosaikError> {
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
        let input: InputData = serde_json::from_value(request.args[1].clone())?;

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
        Ok(())
    }

    #[test]
    fn test_parse_get_data_request() -> Result<(), MosaikError> {
        let valid_request = r#"[0, 1, ["get_data", [{"eid_1": ["attr_1", "attr_2"]}], {}]]"#;
        let mut outputs = Map::new();
        outputs.insert("eid_1".to_string(), json!(vec!["attr_1", "attr_2"]));
        let expected = Request {
            msg_id: 1,
            method: "get_data".to_string(),
            args: vec![json!(outputs)],
            kwargs: Map::new(),
        };
        assert_eq!(parse_json_request(valid_request)?, expected);
        Ok(())
    }

    // ------------------------------------------------------------------------
    // Tests for `serialize`
    // ------------------------------------------------------------------------

    #[test]
    fn test_serialize_request() {
        let request = Request {
            msg_id: 123,
            method: "my_func".to_string(),
            args: vec![json!("hello"), json!("world")],
            kwargs: {
                let mut map = Map::new();
                map.insert("times".to_string(), json!(23))
                    .unwrap_or_default();
                map
            },
        };
        let ser_request = json!(request);
        let expect_content = json!(["my_func", ["hello", "world"], {"times": 23}]);
        assert_eq!(ser_request, expect_content);

        let actual = MosaikMessage {
            msg_type: MSG_TYPE_REQUEST,
            id: request.msg_id,
            content: json!(request),
        }
        .serialize_to_vec();
        let expect = json!([MSG_TYPE_REQUEST as u8, request.msg_id, expect_content]);
        assert_eq!(
            serde_json::from_slice::<Value>(&actual[4..]).unwrap(),
            expect
        );
    }

    #[test]
    fn test_serialize_response_success_to_vec() {
        let msg_type = MSG_TYPE_REPLY_SUCCESS;
        let id = 1u64;
        let content = json!("the return value");

        let expected_response = to_vec(&json!([1 as u8, id, content])).unwrap();
        let actual_response = MosaikMessage {
            msg_type,
            id,
            content,
        }
        .serialize_to_vec();

        // NOTE mosaik tutorial is wrong and has 2 bytes too many (should be 24B)
        assert_eq!(actual_response[0..4], vec![0x00, 0x00, 0x00, 0x18]);
        assert_eq!(actual_response.len(), 4 + 0x18);
        assert_eq!(actual_response[4..], expected_response);
    }

    #[test]
    fn test_serialize_response_failure_to_vec() {
        let msg_type = MSG_TYPE_REPLY_FAILURE;
        let id = 1;
        let content = json!("Error in your code line 23: ...");

        let expected_response = to_vec(&json!([2 as u8, id, content])).unwrap();
        let actual_response = MosaikMessage {
            msg_type,
            id,
            content,
        }
        .serialize_to_vec();

        // NOTE mosaik Tutorial has 2 bytes too many (should be 39B)
        assert_eq!(actual_response[..4], vec![0x00, 0x00, 0x00, 0x27]);
        assert_eq!(actual_response.len(), 4 + 0x27);
        assert_eq!(actual_response[4..], expected_response);
    }

    #[test]
    #[ignore]
    fn test_serialize_response_error_to_vec() {
        let expect = to_vec(&json!([
            MSG_TYPE_REPLY_FAILURE,
            123,
            "Failed to serialize a vector from the response to MessageID 123"
        ]))
        .unwrap();
        let mut map = HashMap::new();
        map.insert(12, "some failing value");
        let actual = MosaikMessage {
            msg_type: MSG_TYPE_REPLY_SUCCESS,
            id: 123,
            content: json!(map),
            // FIXME json!(vec![0u64; usize::MAX]), // this Error occurs before the handling and will panic!
        }
        .serialize_to_vec();
        // TODO how to test for serialize Error?
        assert_eq!(actual.len(), 4 + expect.len());
        assert_eq!(actual[4..], expect);
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
    fn test_handle_request_init_success() -> Result<(), MosaikError> {
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
            .returning(|_, _, _| Meta::new("3.0", SimulatorType::default(), HashMap::new()));

        let payload = json!(Meta::new("3.0", SimulatorType::default(), HashMap::new()));
        let actual_response = handle_request(&mut simulator, &request);
        assert_eq!(
            actual_response,
            Response::Reply(MosaikMessage {
                msg_type: MSG_TYPE_REPLY_SUCCESS,
                id: request.msg_id,
                content: payload
            })
        );

        Ok(())
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

        let actual = handle_request(&mut mock_simulator, &request);
        let expected = Response::Reply(MosaikMessage {
            msg_type: MSG_TYPE_REPLY_FAILURE,
            id: request.msg_id,
            content: json!("Serde JSON Error: invalid type: integer `0`, expected a string"),
        });
        // TODO do we want a more concise Error message here?
        assert_eq!(actual, expected);
    }

    /* TODO should Simulator functions return Results to make Error Handling available to Users? -> SimulatorError Type?
    #[test]
    fn test_handle_request_init_failure_sim() -> Result<(), MosaikError> {
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
            .returning(|_, _, _| {
                Err(MosaikError::ParseError(
                    "Failed to initialize simulator".to_string(),
                ))
            });

        let expected_response = Response::Failure(vec![70, 97, 105, 108, 101, 100]);
        let actual_response = handle_request(&mut simulator, request)?;

        assert_eq!(actual_response, expected_response);
        Ok(())
    }*/

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
            msg_type: MSG_TYPE_REPLY_SUCCESS,
            id: request.msg_id,
            content: serde_json::to_value(&vec![cr.clone()]).unwrap(),
        };
        let mut mock_simulator = MockMosaikApi::new();
        mock_simulator
            .expect_create()
            .once()
            .with(eq(1), eq("Grid".to_string()), eq(request.kwargs.clone()))
            .returning(move |_, _, _| vec![cr.clone()]);

        let result = handle_request(&mut mock_simulator, &request);
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
            msg_type: MSG_TYPE_REPLY_SUCCESS,
            id: request.msg_id,
            content: serde_json::Value::Null,
        };
        let mut mock_simulator = MockMosaikApi::new();
        mock_simulator
            .expect_setup_done()
            .once()
            .with()
            .returning(move || ());

        let result = handle_request(&mut mock_simulator, &request);
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
                json!({"node_1": {"P": {"full_id1":20, "full_id2":3.14}, "Q": {"full_id1":3,"full_id2": -2.5}},
                       "node_2": {"P": {"full_id1":42}, "Q": {"full_id1":-23.2}}}),
                json!(3600),
            ],
            kwargs: Map::new(),
        };

        let expect = MosaikMessage {
            msg_type: MSG_TYPE_REPLY_SUCCESS,
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
            .returning(move |_, _, _| Some(120));

        let result = handle_request(&mut mock_simulator, &request);
        assert_eq!(result, Response::Reply(expect));
    }

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
            msg_type: MSG_TYPE_REPLY_SUCCESS,
            id: request.msg_id,
            content: json!({"branch_0": {"I": 42.5}}),
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
                let mut outer = HashMap::new();
                outer.insert("branch_0".to_string(), {
                    let mut inner = HashMap::new();
                    inner.insert("I".to_string(), json!(42.5));
                    inner
                });
                outer
            });

        let result = handle_request(&mut mock_simulator, &request);
        assert_eq!(result, Response::Reply(expect));
    }

    // Request:

    // ["stop", [], {}]

    // Reply:

    //     no reply required

    // ------------------------------------------------------------------------

    #[test]
    fn untyped_example() -> serde_json::Result<()> {
        // FIXME christoph, what is this code testing?
        // Some JSON input data as a &str. Maybe this comes from the user.
        let data = r#"
        [0, 1, ["my_func", ["hello", "world"], {"times": 23}]]"#;

        // Parse the string of data into serde_json::Value.
        let payload = match serde_json::from_str(data)? {
            Value::Array(vecs) if vecs.len() >= 3 => vecs,
            _ => panic!("error!"),
        };

        assert_eq!(payload[0], 0);
        let id: u64 = payload[1].as_u64().unwrap();
        assert_eq!(id, 1);
        if let Value::Array(call) = &payload[2] {
            let method: &str = call[0].as_str().unwrap();
            assert_eq!("my_func", method);
            if let Value::Array(args) = &call[1] {
                assert_eq!(args, &vec!["hello".to_string(), "world".to_string()]);
            }
            if let Value::Object(kwargs) = &call[2] {
                let map = json!({"times": 23});
                info!("{:?}", map);
                assert_eq!(*kwargs.get("times").unwrap(), map["times"]);
            }
        }

        // Access parts of the data by indexing with square brackets.
        // info!(
        //     "Please call {} at the number {}",
        //     payload["name"], payload["phones"][0]
        // );

        Ok(())
    }

    #[test]
    fn to_bytes() {
        let _v = json!(["an", "array"]);
        let _data1 = r#"[1, 1,
        {
            "api_version": "3.0",
            "type": "time-based",
            "models":{
                "ExampleModel":{
                    "public": true,
                    "params": ["init_p_mw_pv"],
                    "attrs": ["val", "kw"]
                    }
                }
            }]"#;

        let data = r#"[1, 1, ["my_func", ["hello", "world"], {"times": 23}]]"#;
        let data_value: Value = serde_json::from_str(data).unwrap();
        let vect = to_vec(&data_value);
        let vect_unwrapped = vect.unwrap();
        let vect_unwrapped_length = vect_unwrapped.len();
        let length_u32 = vect_unwrapped_length as u32;
        let _big_endian = length_u32.to_be_bytes();

        debug!("the length of r#: {}", data.len());
        debug!("r# as string: {:?}", vect_unwrapped);
        debug!("number of bytes: {}", vect_unwrapped_length);
        /*let data_bytes = data.bytes();
        let data_bytes_len = data_bytes.len();


        //info!("The length of the array: {}", length);*/
    }
    #[test]
    #[ignore]
    fn request_example() {
        let _data = r#"[0, 1, ["my_func", ["hello", "world"], {"times": 23}]]"#;
        let _full_data =
            r#"\x00\x00\x00\x36[1, 1, ["my_func", ["hello", "world"], {"times": 23}]]"#;

        todo!();
    }

    #[test]
    #[ignore]
    fn return_success() {
        let _data = r#"[1, 1, "the return value"]"#;
        let _full_data = r#"\x00\x00\x00\x1a[1, 1, "the return value"]"#;

        todo!();
    }

    #[test]
    #[ignore]
    fn return_error() {
        let _data = r#"[2, 1, "Error in your code line 23: ..."]"#;
        let _full_data = r#"\x00\x00\x00\x29[2, 1, "Error in your code line 23: ..."]"#;

        todo!()
    }

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

    #[test]
    #[ignore]
    #[should_panic]
    fn test_deserialize_u8_to_enum() {
        #[repr(u8)]
        #[derive(Deserialize, PartialEq, Debug)]
        enum MyEnum {
            A = 0,
            B = 1,
        }

        let actual: MyEnum = serde_json::from_value(json!(0u8)).unwrap();
        assert_eq!(actual, MyEnum::A);
    }
}
