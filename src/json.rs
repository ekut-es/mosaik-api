use std::collections::HashMap;

use log::*;
use serde_json::{json, map::Map, to_vec, Value};

use thiserror::Error;

use crate::{AttributeId, Eid, MosaikApi};

#[derive(Error, Debug)]
pub enum MosaikError {
    #[error("Parsing Mosaik Payload: {0}")]
    ParseError(String),
    #[error("Parsing Error")]
    Serde(#[from] serde_json::Error),
}

pub enum Response {
    Successfull(Vec<u8>),
    Stop(Vec<u8>),
    None,
}

pub fn parse_request(data: String) -> Result<Request, MosaikError> {
    // Parse the string of data into serde_json::Value.
    let payload = match serde_json::from_str(&data)? {
        Value::Array(vecs) if vecs.len() == 3 => vecs,
        e => {
            return Err(MosaikError::ParseError(format!("Invalid Payload: {:?}", e)));
        }
    };

    if payload[0] != 0 {
        return Err(MosaikError::ParseError(format!(
            "Payload is not a request: {:?}",
            payload
        )));
    }

    let id: u64 = payload[1].as_u64().unwrap();

    match payload[2].clone() {
        Value::Array(call) if call.len() == 3 => {
            match (call[0].as_str(), call[1].clone(), call[2].clone()) {
                (Some(method), Value::Array(args), Value::Object(kwargs)) => Ok(Request {
                    id,
                    method: method.to_string(),
                    args,
                    kwargs,
                }),
                (e1, e2, e3) => Err(MosaikError::ParseError(format!(
                    "Payload is not a valid request: {:?} | {:?} | {:?}",
                    e1, e2, e3
                ))),
            }
        }
        e => Err(MosaikError::ParseError(format!(
            "Payload doesn't have valid method, args, kwargs Array: {:?}",
            e
        ))),
    }
}

pub fn handle_request<T: MosaikApi>(request: Request, simulator: &mut T) -> Response {
    let content: Value = match request.method.as_ref() {
        "init" => simulator.init(
            request.args[0]
                .as_str()
                .unwrap_or("No Simulation ID from the request.")
                .to_string(),
            Some(request.kwargs),
        ),
        "create" => Value::from(simulator.create(
            request.args[0].as_u64().unwrap_or_default() as usize,
            request.args[1].clone(),
            Some(request.kwargs),
        )),
        "step" => Value::from(simulator.step(
            request.args[0].as_u64().unwrap_or_default() as usize,
            inputs_to_hashmap(request.args[1].clone()),
        )),
        "get_data" => Value::from(simulator.get_data(outputs_to_hashmap(request.args))),
        "setup_done" => {
            simulator.setup_done();
            json!(null)
        }
        "stop" => {
            simulator.stop();
            return match to_vec_helper(json!(null), request.id) {
                Some(vec) => Response::Stop(vec),
                None => Response::None,
            };
        }
        e => {
            error!(
                "A different not yet implemented method {:?} got requested. Therefore the simulation should most likely stop now",
                e
            );
            return Response::None; //TODO: see issue #2 but most likely it should stay as it is instead of return json!(null)
        }
    };

    match to_vec_helper(content, request.id) {
        Some(vec) => Response::Successfull(vec),
        None => Response::None,
    }
}

fn to_vec_helper(content: Value, id: u64) -> Option<Vec<u8>> {
    //struct Response:
    //msg_type: MsgType,
    //id: usize,
    //payload: String,
    let response: Value = Value::Array(vec![json!(1), json!(id), content]);

    match to_vec(&response) {
        // Make a u8 vector with the data
        Ok(mut vect_unwrapped) => {
            let mut big_endian = (vect_unwrapped.len() as u32).to_be_bytes().to_vec();
            big_endian.append(&mut vect_unwrapped);
            Some(big_endian) //return the final response
        }
        Err(e) => {
            error!("Failed to make an Vector with the response: {}", e);
            None
        }
    }
}

///Transform the requested map to hashmap of Id to a mapping
fn inputs_to_hashmap(inputs: Value) -> HashMap<Eid, Map<AttributeId, Value>> {
    let mut hashmap = HashMap::new();
    if let Value::Object(eid_map) = inputs {
        for (eid, attr_values) in eid_map.into_iter() {
            if let Value::Object(attrid_map) = attr_values {
                hashmap.insert(eid, attrid_map);
            }
        }
    }
    hashmap
}

///Transform the requested map to hashmap of Id to a vector
fn outputs_to_hashmap(outputs: Vec<Value>) -> HashMap<Eid, Vec<AttributeId>> {
    let mut hashmap = HashMap::new();
    for output in outputs {
        if let Value::Object(eid_map) = output {
            for (eid, attr_id_array) in eid_map.into_iter() {
                if let Value::Array(attr_id) = attr_id_array {
                    hashmap.insert(
                        eid,
                        attr_id
                            .iter()
                            .filter_map(|x| x.as_str().map(|x| x.to_string()))
                            .collect(),
                    );
                }
            }
        }
    }
    hashmap
}
//TODO: Clean this up and remove it?
// enum MsgType {
//     REQ,
//     SUCCESS,
//     ERROR,
// }

#[derive(Debug)]
pub struct Request {
    id: u64,
    method: String,
    args: Vec<Value>,
    kwargs: Map<String, Value>,
}

#[cfg(test)]
mod tests {
    use log::*;

    use serde_json::{json, to_vec, Result, Value};
    #[test]
    fn untyped_example() -> Result<()> {
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
                println!("{:?}", map);
                assert_eq!(*kwargs.get("times").unwrap(), map["times"]);
            }
        }

        // Access parts of the data by indexing with square brackets.
        // println!(
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
            "api_version": "2.2",
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


        //println!("The length of the array: {}", length);*/
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
