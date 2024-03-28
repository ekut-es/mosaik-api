use std::collections::HashMap;

use log::*;
use serde::Deserialize;
use serde_json::{json, map::Map, to_vec, Value};

use thiserror::Error;

use crate::{MosaikApi, OutputRequest};

#[derive(Error, Debug)]
pub enum MosaikError {
    #[error("Parsing Mosaik Payload: {0}")]
    ParseError(String),
    #[error("Parsing Error: {0}")]
    Serde(#[from] serde_json::Error),
}

#[derive(Debug, Deserialize)]
pub struct MosaikPayload {
    msg_type: u8,
    id: u64,
    content: Value,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct Request {
    #[serde(skip)]
    msg_id: u64,
    method: String,
    args: Vec<Value>,
    kwargs: Map<String, Value>,
}

pub enum Response {
    Successful(Vec<u8>),
    Failure(Vec<u8>),
    Stop(Vec<u8>),
    None,
}

pub fn parse_request(data: String) -> Result<Request, MosaikError> {
    // Parse the string of data into serde_json::Value.
    let payload: MosaikPayload = serde_json::from_str(&data)?;

    if payload.msg_type != 0 {
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
    request: Request,
    simulator: &mut T,
) -> Result<Response, MosaikError> {
    let content: Value = match request.method.as_ref() {
        "init" => simulator.init(
            serde_json::from_value(request.args[0].clone())?,
            // get time_resolution from kwargs and put the rest in sim_params map
            request
                .kwargs
                .get("time_resolution")
                .and_then(|x| x.as_f64())
                .unwrap_or(1.0f64),
            request.kwargs,
        ),
        "create" => Value::from(simulator.create(
            serde_json::from_value(request.args[0].clone())?,
            serde_json::from_value(request.args[1].clone())?,
            request.kwargs,
        )),
        "step" => Value::from(simulator.step(
            serde_json::from_value(request.args[0].clone())?,
            serde_json::from_value(request.args[1].clone())?,
            serde_json::from_value(request.args[2].clone())?,
        )), // add handling of optional return
        "get_data" => serde_json::to_value(
            simulator.get_data(serde_json::from_value(request.args[0].clone())?),
        )?,
        "setup_done" => {
            simulator.setup_done();
            json!(null)
        }
        "stop" => {
            simulator.stop();
            return match to_vec_helper(json!(null), request.msg_id) {
                Some(vec) => Ok(Response::Stop(vec)),
                None => Ok(Response::None),
            };
        }
        e => {
            error!(
                "A different not yet implemented method {:?} got requested. Therefore the simulation should most likely stop now",
                e
            );
            return Ok(Response::None); //TODO: see issue #2 but most likely it should stay as it is instead of return json!(null)
        }
    };

    match to_vec_helper(content, request.msg_id) {
        Some(vec) => Ok(Response::Successful(vec)),
        None => {
            let response: Value = Value::Array(vec![
                json!(2),
                json!(request.msg_id),
                Value::String("Stack Trace/Error Message".to_string()),
            ]);
            Ok(Response::Failure(to_vec(&response).unwrap()))
        }
    }
}

fn to_vec_helper(content: Value, id: u64) -> Option<Vec<u8>> {
    //struct Response:
    //msg_type: MsgType,
    //id: usize,
    //payload: String,
    // FIXME is this function necessary? or check Result in handle_request directly
    let response: Value = Value::Array(vec![json!(1), json!(id), content]);

    match to_vec(&response) {
        // Make a u8 vector with the data
        Ok(mut vect_unwrapped) => {
            let mut big_endian = (vect_unwrapped.len() as u32).to_be_bytes().to_vec();
            big_endian.append(&mut vect_unwrapped);
            Some(big_endian) //return the final response
        }
        Err(e) => {
            error!("Failed to create a vector with the response: {}", e);
            None
        }
    }
}

//TODO: Clean this up and remove it?
// enum MsgType {
//     REQ,
//     SUCCESS,
//     ERROR,
// }

#[cfg(test)]
mod tests {
    use crate::types::InputData;

    use super::*;
    use serde_json::{json, to_vec, Value};

    #[test]
    fn parse_request_valid() -> Result<(), MosaikError> {
        let valid_request = r#"[0, 1, ["my_func", ["hello", "world"], {"times": 23}]]"#.to_string();
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
        assert_eq!(parse_request(valid_request)?, expected);

        Ok(())
    }

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
        ]"#
        .to_string();

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
        let p = parse_request(valid_request)?;
        println!("got until here");
        let input: InputData = serde_json::from_value(p.args[1].clone())?;
        println!("input: {:?}", input);

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
        assert_eq!(p, expected);
        Ok(())
    }

    #[test]
    fn parse_get_data_request() -> Result<(), MosaikError> {
        let valid_request =
            r#"[0, 1, ["get_data", [{"eid_1": ["attr_1", "attr_2"]}], {}]]"#.to_string();
        let mut outputs = Map::new();
        outputs.insert("eid_1".to_string(), json!(vec!["attr_1", "attr_2"]));
        let expected = Request {
            msg_id: 1,
            method: "get_data".to_string(),
            args: vec![json!(outputs)],
            kwargs: Map::new(),
        };
        assert_eq!(parse_request(valid_request)?, expected);
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
