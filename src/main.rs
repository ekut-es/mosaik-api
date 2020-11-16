use jsonrpc_core::*;

pub fn main(){
    //build tcp connect

    let mut io = IoHandler::default();

    io.add_method("say_hello", |_params: Params| {
        Ok(Value::String("hello".into()))
    });

    let request = r#"{"jsonrpc": "2.0", "method": "say_hello", "params": [42, 23], "id": 1}"#; //an die Form von Mosaik anpassen
    let resonse = r#"{"jsonrpc": "2.0", "result": "hello", "id":1}"#; //an die Form von Mosaik anpassen

    assert_eq!(io.handle_request_sync(request), Some(response.to_owned()));



}


