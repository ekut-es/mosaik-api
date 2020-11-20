use serde_json::{json, map::Map, Result, Value};

// https://book.async.rs/tutorial/all_together.html

enum MsgType {
    REQ,
    SUCCESS,
    ERROR,
}

struct Request {
    msg_type: MsgType,
    id: usize,
    method: String,
    args: Vec<String>,
    kwargs: Map<String, Value>,
}

struct Response {
    msg_type: MsgType,
    id: usize,
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Result, Value};
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
        use std::iter::FromIterator;
        if let Value::Array(call) = &payload[2] {
            let method: &str = call[0].as_str().unwrap();
            assert_eq!("my_func", method);
            if let Value::Array(args) = &call[1] {
                assert_eq!(args, &vec!["hello".to_string(), "world".to_string()]);
            }
            if let Value::Object(kwargs) = &call[2] {
                assert_eq!(
                    kwargs.get("times"),
                    serde_json::map::Map::from_iter(
                        vec![("times".to_string(), json!(23))].into_iter()
                    )
                    .get("times")
                );
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
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }

    #[test]
    fn request_example() {
        let data = r#"[0, 1, ["my_func", ["hello", "world"], {"times": 23}]]"#;
        let full_data = r#"\x00\x00\x00\x36[1, 1, ["my_func", ["hello", "world"], {"times": 23}]]"#;

        todo!();

        assert_eq!(2 + 2, 4);
    }

    #[test]
    fn return_success() {
        let data = r#"[1, 1, "the return value"]"#;
        let full_data = r#"\x00\x00\x00\x1a[1, 1, "the return value"]"#;

        todo!();

        assert_eq!(2 + 2, 4);
    }

    #[test]
    fn return_error() {
        let data = r#"[2, 1, "Error in your code line 23: ..."]"#;
        let full_data = r#"\x00\x00\x00\x29[2, 1, "Error in your code line 23: ..."]"#;
        assert_eq!(2 + 2, 4);

        todo!()
    }

    #[test]
    fn init() {
        let data = r#"[2, 1, "Error in your code line 23: ..."]"#;
        let full_data = r#"\x00\x00\x00\x29[2, 1, "Error in your code line 23: ..."]"#;
        assert_eq!(2 + 2, 4);

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
