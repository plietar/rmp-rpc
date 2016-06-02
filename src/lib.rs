#[macro_use] extern crate quick_error;
extern crate rmp;

use std::io;
use rmp::Value;

#[derive(Debug)]
pub struct Error(Option<Box<std::error::Error>>);

#[derive(Debug)]
struct Request {
    msgid: rmp::value::Integer,
    method: String,
    params: Vec<Value>,
}

#[derive(Debug)]
struct Response {
    msgid: rmp::value::Integer,
    error: Value,
    result: Value,
}

quick_error! {
    #[derive(Debug)]
    enum RequestDecodeError {
        InvalidMessageType {}
        TypeMismatch {}
        InvalidLength {}
    }
}

impl Request {
    pub fn decode(value: rmp::Value) -> Result<Request, RequestDecodeError> {
        use rmp::Value::*;
        use RequestDecodeError::*;
        use rmp::value::Integer::*;

        let values = match value {
            Array(values) => values,
            _ => return Err(TypeMismatch),
        };

        let mut values = values.into_iter();
        let typ = values.next();
        let msgid = values.next();
        let method = values.next();
        let params = values.next();
        let end = values.next();

        match (typ, msgid, method, params, end) {
            (Some(Integer(typ)),
             Some(Integer(msgid)),
             Some(String(method)),
             Some(Array(params)),
             None) => {
                if typ == U64(0) || typ == I64(0) {
                    Ok(Request {
                        msgid: msgid,
                        method: method,
                        params: params,
                    })
                } else {
                    Err(InvalidMessageType)
                }
            }
            (Some(_), Some(_), Some(_), Some(_), None) => Err(TypeMismatch),
            _ => Err(InvalidLength)
        }
    }
}

impl Response {
    pub fn encode(self) -> rmp::Value {
        use rmp::Value::*;
        use rmp::value::Integer::U64;

        Array(vec![
              Integer(U64(1)),
              Integer(self.msgid),
              self.error,
              self.result,
        ])
    }
}

impl <T: std::error::Error + 'static> From<T> for Error {
    fn from(error: T) -> Error {
        Error(Some(Box::new(error)))
    }
}

pub trait Handler {
    fn request(&mut self, method: &str, params: Vec<Value>)
        -> Result<Value, Value>;
}

pub struct Server<H: Handler> {
    handler: H,
}

impl <H: Handler> Server<H> {
    pub fn new(handler: H) -> Server<H> {
        Server {
            handler: handler,
        }
    }

    pub fn serve<S: io::Read + io::Write>(&mut self, mut socket: S) {
        loop {
            socket = self.serve_one(socket).unwrap();
        }
    }

    pub fn serve_one<S: io::Read + io::Write>(&mut self, mut socket: S)
        -> Result<S, Error> {

        let request = try!(rmp::decode::read_value(&mut socket));
        let request = try!(Request::decode(request));

        let result = self.handler.request(&request.method, request.params);

        let (value, error) = match result {
            Ok(value) => (value, rmp::Value::Nil),
            Err(error) => (rmp::Value::Nil, error),
        };

        let response = Response {
            msgid: request.msgid,
            error: error,
            result: value,
        };

        try!(rmp::encode::value::write_value(&mut socket, &response.encode()));

        Ok(socket)
    }
}
