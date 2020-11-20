use jsonrpc_core::*;
use jsonrpc_http_server::jsonrpc_core::{IoHandler, Value, Params};
use jsonrpc_http_server::ServerBuilder;
use std::net::{TcpStream};
use std::io::{Read, Write};
use std::str::from_utf8;

use std::{thread, time};

pub fn main() {    
    /*
    let ten_millis = time::Duration::from_millis(100);
    
    thread::sleep(ten_millis);
    */
    //tcp();
    
   
    
    let mut io = IoHandler::new();

    io.add_method("say_hello", |_params: Params| {
        println!("TEST_sayhello");
        Ok(Value::String("hello".into()/*.to_owned() */ ))
    });

    io.add_method("init", |_params: Params|{
        println!("Test: {:?}", _params);
        Ok(Value::String("hello".into()/*.to_owned() */ ))
    });
/*
    io.add_method("create", |_params: Params|{
        Ok(Value::String("hello".into()/*.to_owned() */ ))
    });

    io.add_method("step", |_params: Params|{
        Ok(Value::String("hello".into()/*.to_owned() */ ))
    });
    
    io.add_method("setup_done", |_params: Params|{
        Ok(Value::String("hello".into()/*.to_owned() */ ))
    });

    io.add_method("stop", |_params: Params|{
        Ok(Value::String("hello".into()/*.to_owned() */ ))
    });

    io.add_method("get_progress", |_params: Params|{
        Ok(Value::String("hello".into()/*.to_owned() */ ))
    });

    io.add_method("get_related_entities", |_params: Params|{
        Ok(Value::String("hello".into()/*.to_owned() */ ))
    });
    
    io.add_method("get_data", |_params: Params|{
        Ok(Value::String("hello".into()/*.to_owned() */ ))
    });

    io.add_method("set_data", |_params: Params|{
        Ok(Value::String("hello".into()/*.to_owned() */ ))
    });*/

    //let request = r#"{"jsonrpc": "2.0", "method": "say_hello", "params": [42, 23], "id": 1}"#; //an die Form von Mosaik anpassen
    let response = r#"{"jsonrpc":"2.0","result":"hello","id":1}"#; //an die Form von Mosaik anpassen

    //assert_eq!(io.handle_request_sync(request), Some(response.to_owned()));

    println!("Rust Programm beendet");

    let server = ServerBuilder::new(io)
		.threads(3)
		.start_http(&"127.0.0.1:3030".parse().unwrap())
		.unwrap();

	server.wait();
}

fn tcp(){
    
    match TcpStream::connect("localhost:5555") {
        Ok(mut stream) => {
            println!("Successfully connected to server in port 5555");

            let msg = b"Hello!";
            //json::parse(r#"{"1": "hello"  }"#).unwrap();
            

            //stream.write(msg).unwrap();
            println!("Sent Hello, awaiting reply...");

            let mut data = [0 as u8; 6]; // using 6 byte buffer
            match stream.read_exact(&mut data) {
                Ok(_) => {
                    if &data == msg {
                        println!("Reply is ok!");
                    } else {
                        let text = from_utf8(&data).unwrap();
                        println!("Unexpected reply: {}", text);
                    }
                },
                Err(e) => {
                    println!("Failed to receive data: {}", e);
                }
            }
        },
        Err(e) => {
            println!("Failed to connect: {}", e);
        }
    }
    println!("Terminated.");

}

pub fn start_simulator(){
    let ok: i32 = 0;
    let err: i32 = 1;



}
