use jsonrpc_core::*;
use jsonrpc_http_server::jsonrpc_core::{IoHandler, Value, Params};
use jsonrpc_http_server::ServerBuilder;

pub fn main() {
    //build tcp connect

    let mut io = IoHandler::new();

    io.add_method("say_hello", |_params: Params| {
        Ok(Value::String("hello".into()/*.to_owned() */ ))
    });
    

    let request = r#"{"jsonrpc": "2.0", "method": "say_hello", "params": [42, 23], "id": 1}"#; //an die Form von Mosaik anpassen
    let response = r#"{"jsonrpc":"2.0","result":"hello","id":1}"#; //an die Form von Mosaik anpassen

    assert_eq!(io.handle_request_sync(request), Some(response.to_owned()));

    println!("Rust Programm beendet");

    let server = ServerBuilder::new(io)
		.threads(3)
		.start_http(&"127.0.0.1:5555".parse().unwrap())
		.unwrap();

	server.wait();
}
