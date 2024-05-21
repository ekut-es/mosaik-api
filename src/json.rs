use log::*;
use serde::Deserialize;
use serde_json::{json, map::Map, to_vec, Value};

use thiserror::Error;

use crate::MosaikApi;

#[derive(Error, Debug)]
pub enum MosaikError {
    #[error("Parsing Mosaik Payload: {0}")]
    ParseError(String),
    #[error("Parsing Error: {0}")]
    Serde(#[from] serde_json::Error),
}

#[derive(Debug, Deserialize)]
pub struct MosaikMessage {
    pub msg_type: u8,
    pub id: MessageID,
    pub content: Value,
}

pub const MSG_TYPE_REQUEST: u8 = 0;
pub const MSG_TYPE_REPLY_SUCCESS: u8 = 1;
pub const MSG_TYPE_REPLY_FAILURE: u8 = 2;

type MessageID = u64;

#[derive(Debug, Deserialize, PartialEq)]
pub struct Request {
    #[serde(skip)]
    msg_id: MessageID,
    method: String,
    args: Vec<Value>,
    kwargs: Map<String, Value>,
}

#[derive(Debug, PartialEq)]
pub enum Response {
    Successful((MessageID, Value)),
    Failure((MessageID, String)),
    Stop,
    NoReply,
}

pub fn parse_json_request(data: &str) -> Result<Request, MosaikError> {
    // Parse the string of data into serde_json::Value.
    let payload: MosaikMessage = serde_json::from_str(&data)?;

    if payload.msg_type != MSG_TYPE_REQUEST {
        return Err(MosaikError::ParseError(format!(
            "Payload is not a request: {:?}",
            payload
        )));
    }

    let mut request: Request = serde_json::from_value(payload.content)?;
    request.msg_id = payload.id;
    Ok(request)
}

pub fn handle_request<T: MosaikApi>(
    simulator: &mut T,
    request: &Request,
) -> Result<Response, MosaikError> {
    // TODO include error handling
    let content: Value = match request.method.as_ref() {
        "init" => handle_init(simulator, &request)?,
        "create" => handle_create(simulator, &request)?,
        "step" => handle_step(simulator, &request)?,
        "get_data" => handle_get_data(simulator, &request)?,
        "setup_done" => {
            simulator.setup_done();
            Value::Null
        }
        "stop" => {
            debug!("Received stop command!");
            simulator.stop();
            return Ok(Response::Stop);
        }
        method => {
            error!(
                "Unimplemented method {:?} requested. Simulation should most likely stop now",
                method
            );
            return Ok(Response::Failure((
                request.msg_id,
                format!(
                    "Unimplemented method {:?} requested. Simulation should most likely stop now",
                    method
                ),
                // TODO: see issue #2, maybe we should stop the API here
            )));
        }
    };

    Ok(Response::Successful((request.msg_id, content)))
}

fn handle_init<T: MosaikApi>(simulator: &mut T, request: &Request) -> Result<Value, MosaikError> {
    Ok(serde_json::to_value(
        simulator.init(
            serde_json::from_value(request.args[0].clone())?,
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

pub fn serialize_mosaik_message(payload: MosaikMessage) -> Vec<u8> {
    let response: Value = json!([payload.msg_type as u8, payload.id, payload.content]);
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
                payload.id, e
            );
            let error_message = format!(
                "Failed to serialize a vector from the response to MessageID {}",
                payload.id
            );
            let error_response = json!([MSG_TYPE_REPLY_FAILURE, payload.id, error_message]);
            to_vec(&error_response).unwrap() // FIXME unwrap should be safe, because we know the error message is a short enough string
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{InputData, Meta, SimulatorType};
    use crate::MockMosaikApi;

    use mockall::predicate::*;
    use serde_json::{json, to_vec, Value};
    use std::{
        any::{Any, TypeId},
        collections::HashMap,
    };

    // Tests for `parse_json_request`

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
        assert_eq!(result.unwrap(), expected);

        Ok(())
    }

    #[test]
    fn test_parse_invalid_request() -> Result<(), MosaikError> {
        let data = r#"invalid request data"#;
        let result = parse_json_request(data);
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn test_parse_success_reply() -> Result<(), MosaikError> {
        let data = r#"[1, 1, ["my_func", ["hello", "world"], {"times": 23}]]"#;
        let result = parse_json_request(data);
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn test_parse_failure_reply() -> Result<(), MosaikError> {
        let data = r#"[2, 1, ["my_func", ["hello", "world"], {"times": 23}]]"#;
        let result = parse_json_request(data);
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn test_parse_invalid_message_type() -> Result<(), MosaikError> {
        let data = r#"["0", 1, ["my_func", ["hello", "world"], {"times": 23}]]"#;
        let result = parse_json_request(data);
        assert!(result.is_err());
        Ok(())
    }

    // Tests for `handle_request`

    #[test]
    fn test_handle_init_success() -> Result<(), MosaikError> {
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
        assert!(actual_response.is_ok());
        assert_eq!(
            actual_response.unwrap(),
            Response::Successful((request.msg_id, payload))
        );

        Ok(())
    }

    #[test]
    #[ignore]
    fn test_handle_request_init() {
        let mut kwargs = Map::new();
        kwargs.insert("time_resolution".to_string(), json!(0.1));
        kwargs.insert("step_size".to_string(), json!(60));
        let request = Request {
            msg_id: 123,
            method: "init".to_string(),
            args: vec![json!("PowerGridSim-0")],
            kwargs: kwargs.clone(),
        };

        let mut mock_simulator = MockMosaikApi::new();
        mock_simulator.expect_init().once().with(
            eq("PowerGridSim-0".to_string()),
            eq(0.1),
            eq(kwargs),
        );
        // TODO check if it returns a Meta object.
        // FIXME .returning(|_, _, _| Meta::new("3.0", SimulatorType::default(), models)); does not work - don't know why
        let result = handle_request(&mut mock_simulator, &request);
        assert!(result.is_ok());
        // TODO check if response is of type SuccessReply
        let response = result.unwrap();
        assert_eq!(response.type_id(), TypeId::of::<Response>());
    }

    #[test]
    #[ignore]
    fn test_handle_request_create() {
        let mut map = Map::new();
        map.insert("init_val".to_string(), json!(42))
            .unwrap_or_default();

        let request = Request {
            msg_id: 1,
            method: "create".to_string(),
            args: vec![json!(2), json!("ExampleModel")],
            kwargs: map.clone(),
        };

        let mut mock_simulator = MockMosaikApi::new();
        mock_simulator.expect_create().with(
            eq(2 as usize),
            eq("ExampleModel".to_string()),
            eq(map),
        );

        let result = handle_create(&mut mock_simulator, &request);
        assert!(result.is_ok());
    }

    // Tests for `serialize_response`

    #[test]
    fn test_serialize_response_success() {
        let msg_type = MSG_TYPE_REPLY_SUCCESS;
        let id = 123u64;
        let content = json!("Success");

        let expected_response = vec![
            91, 49, 44, 49, 50, 51, 44, 34, 83, 117, 99, 99, 101, 115, 115, 34, 93,
        ];
        let actual_response = serialize_mosaik_message(MosaikMessage {
            msg_type,
            id,
            content,
        });
        // check first 4 bytes to match header
        assert_eq!(
            actual_response[0..4],
            (expected_response.len() as u32).to_be_bytes()
        );
        assert_eq!(actual_response.len(), 4 + expected_response.len());
        assert_eq!(actual_response[4..], expected_response);
    }

    #[test]
    fn test_serialize_response_failure() {
        let msg_type = MSG_TYPE_REPLY_FAILURE;
        let id = 456;
        let content = json!("Failure");

        let expected_response = vec![
            91, 50, 44, 52, 53, 54, 44, 34, 70, 97, 105, 108, 117, 114, 101, 34, 93,
        ]; // == to_vec(&json!([2 as u8, msg_id, payload]))
        let actual_response = serialize_mosaik_message(MosaikMessage {
            msg_type,
            id,
            content,
        });

        assert_eq!(
            actual_response[..4],
            (expected_response.len() as u32).to_be_bytes()
        );
        assert_eq!(actual_response.len(), 4 + expected_response.len());
        assert_eq!(actual_response[4..], expected_response);
    }

    // TODO test serialize Error, maybe give it own error type.
    // Tests for `handle_request`

    /*
    #[test]
    fn test_handle_init_failure() -> Result<(), MosaikError> {
        let mut simulator = MockMosaikApi::new();
        let request = Request {
            msg_id: 789,
            method: "init".to_string(),
            args: vec![json!("arg1"), json!("arg2")],
            kwargs: Map::new(),
        };

        simulator
            .expect_init()
            .with(eq(json!("arg1")), eq(1.0), eq(Map::new()))
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

    /*#[test]
    fn test_handle_request() -> Result<(), MosaikError> {
        let mut mock_simulator = MockMosaikApi::new();

        let mut models = Map::new();
        models.insert("model_1".to_string(), json!(1)).unwrap();
        models.insert("model_2".to_string(), json!(2)).unwrap();
        models.insert("model_3".to_string(), json!(3)).unwrap();
        mock_simulator
            .expect_init()
            .with(eq(1), eq(0.1), always())
            .returning(|_, _, _| Meta::new("3.0", SimulatorType::default(), models));

        let request = Request {
            msg_id: 0,
            method: "init".to_string(),
            args: vec![json!("hello"), json!("world")],
            kwargs: Map::new(),
        };

        let result = handle_request(&mut simulator, &request);
        assert!(result.is_ok());
        Ok(())
    }*/
    // ----

    #[test]
    fn parse_step_request() -> Result<(), MosaikError> {
        let valid_request = r#"
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
        let result = parse_json_request(valid_request);
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
    fn parse_get_data_request() -> Result<(), MosaikError> {
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

    #[test]
    fn untyped_example() -> serde_json::Result<()> {
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

    #[test]
    #[ignore]
    fn init() {
        let _data = r#"[2, 1, "Error in your code line 23: ..."]"#;
        let _full_data = r#"\x00\x00\x00\x29[2, 1, "Error in your code line 23: ..."]"#;

        todo!()
    }

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

    // Request:

    // ["setup_done", [], {}]

    // Reply:

    // null

    // Request:

    // [
    //     "step",
    //     [
    //         60,
    //         {
    //               "node_1": {"P": [20, 3.14], "Q": [3, -2.5]},
    //               "node_2": {"P": [42], "Q": [-23.2]},
    //         }
    //     ],
    //     {}
    // ]

    // Reply:

    // 120

    // Request:

    // ["get_data", [{"branch_0": ["I"]}], {}]

    // Reply:

    // {
    //     "branch_0": {
    //         "I": 42.5
    //     }
    // }

    // Request:

    // ["stop", [], {}]

    // Reply:

    //     no reply required

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

    // Request:

    // ["get_data", [{"grid_sim_0.branch_0": ["I"]}], {}]

    // Reply:

    // {
    //     "grid_sim_0.branch_0": {
    //         "I": 42.5
    //     }
    // }

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
