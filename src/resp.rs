use actix_web::{HttpResponse, HttpResponseBuilder};

use crate::store::error::StoreError;

type DropResult = Result<crate::Clipboard, StoreError>;

trait DropResponse {
    fn send_clipboard(self, builder: HttpResponseBuilder) -> HttpResponse;
}

struct ResponseHtml(DropResult);
impl DropResponse for ResponseHtml {
    fn send_clipboard(self, mut builder: HttpResponseBuilder) -> HttpResponse {
        builder.content_type("text/html");

        if let Err(err) = self.0 {
            if let Some(public_err) = crate::error::public_error(err) {
                let body = wrap_html(&format!("<p>error: {}</p>", public_err.to_string()));
                return builder.body(body);
            }

            return builder.body(format!("private error"));
        }

        let maybe_body = String::from_utf8(self.0.unwrap().to_vec());
        if let Err(conv_err) = maybe_body {
            let body = wrap_html(&format!("error: {:?}", StoreError::InvalidUtf8(conv_err)));
            return builder.body(body);
        }

        let body = wrap_html(&maybe_body.unwrap());
        builder.body(body)
    }
}

struct ResponsePlain(DropResult);
impl DropResponse for ResponsePlain {
    fn send_clipboard(self, mut builder: HttpResponseBuilder) -> HttpResponse {
        builder.content_type("text/plain");

        if let Err(err) = self.0 {
            if let Some(public_err) = crate::error::public_error(err) {
                let body = format!("error: {}", public_err.to_string());
                return builder.body(body);
            }

            return builder.body(format!("private error"));
        }

        let maybe_body = String::from_utf8(self.0.unwrap().to_vec());
        if let Err(conv_err) = maybe_body {
            let body = format!("error: {:?}", StoreError::InvalidUtf8(conv_err));
            return builder.body(body);
        }

        let body = maybe_body.unwrap();
        builder.body(body)
    }
}

struct ResponseJson(DropResult);
impl DropResponse for ResponseJson {
    fn send_clipboard(self, mut builder: HttpResponseBuilder) -> HttpResponse {
        builder.content_type("application/json");

        if let Err(err) = self.0 {
            if let Some(public_err) = crate::error::public_error(err) {
                let body = serde_json::to_string(&public_err).unwrap();
                return builder.body(body);
            }

            return builder.body(format!("private error"));
        }

        let maybe_body = String::from_utf8(self.0.unwrap().to_vec());
        if let Err(conv_err) = maybe_body {
            let body = serde_json::to_string(&StoreError::InvalidUtf8(conv_err)).unwrap();
            return builder.body(body);
        }

        let body = maybe_body.unwrap();
        builder.body(body)
    }
}

pub const HEADER: &str = r#"<!DOCTYPE html><html><head><meta name=viewport content="width=device-width, initial-scale=1.0"><meta name=keywords content="actix-drop"><meta name=author content=@artnoi><meta charset=UTF-8><link href=https://artnoi.com/style.css rel=stylesheet><title>actix-drop</title></head><body><h1><a href="/">actix-drop</a></h1>"#;
pub const FOOTER: &str = r#"<footer><p><a href="https://github.com/artnoi43/actix-drop">Contribute on Github</a></p></footer></body></html>"#;

pub fn wrap_html(s: &str) -> String {
    format!("{}{}{}", HEADER, s, FOOTER)
}
