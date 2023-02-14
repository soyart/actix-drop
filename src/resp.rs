use actix_web::{HttpResponse, HttpResponseBuilder};
use serde_json::json;

use crate::store::error::{public_error, StoreError};

type DropResult = Result<Option<crate::Clipboard>, StoreError>;

pub trait DropResponseHttp: From<DropResult> {
    const CONTENT_TYPE: &'static str;

    fn format_err(hash: &str, err: StoreError) -> String;
    fn send_clipboard(self, hash: &str, builder: HttpResponseBuilder) -> HttpResponse;
    fn post_clipboard(self, hash: &str, builder: HttpResponseBuilder) -> HttpResponse;
}

pub struct ResponseHtml(DropResult);
pub struct ResponsePlain(DropResult);
pub struct ResponseJson(DropResult);

macro_rules! impl_from_drop_result {
    ( $( $t: ident );+ ) => {
        $(
            impl From<DropResult> for $t {
                fn from(result: DropResult) -> $t {
                    $t(result)
                }
            }

        )*
    }
}

impl_from_drop_result!(ResponseHtml; ResponsePlain; ResponseJson);

impl DropResponseHttp for ResponseHtml {
    const CONTENT_TYPE: &'static str = "text/html";

    fn format_err(hash: &str, err: StoreError) -> String {
        format!(
            "<p>Error for clipboard {hash}: {}</p>",
            extract_error_msg(err)
        )
    }

    fn send_clipboard(self, hash: &str, mut builder: HttpResponseBuilder) -> HttpResponse {
        builder.content_type(Self::CONTENT_TYPE);

        if let Err(err) = self.0 {
            let body = wrap_html(&Self::format_err(hash, err));
            return builder.body(body);
        }

        let maybe_body = String::from_utf8(self.0.unwrap().unwrap().to_vec());
        if let Err(conv_err) = maybe_body {
            let body = wrap_html(&format!("error: {:?}", StoreError::InvalidUtf8(conv_err)));
            return builder.body(body);
        }

        builder.body(wrap_html(&maybe_body.unwrap()))
    }

    fn post_clipboard(self, hash: &str, mut builder: HttpResponseBuilder) -> HttpResponse {
        let body;

        if let Err(err) = self.0 {
            body = format!(
                "<p>Error saving clipboard {hash}: {}</p>",
                extract_error_msg(err)
            );
        } else {
            body = format!(
                r#"<p>Clipboard with hash <code>{hash}</code> created</p>
        <p>The clipboard is now available at path <a href="/drop/{hash}/"><code>/drop/{hash}/</code></a></p>"#
            );
        }

        builder
            .content_type(Self::CONTENT_TYPE)
            .body(wrap_html(&body))
    }
}

impl DropResponseHttp for ResponsePlain {
    const CONTENT_TYPE: &'static str = "text/plain; charset=utf-8";

    fn format_err(hash: &str, err: StoreError) -> String {
        format!("error for clipboard {hash}: {}", extract_error_msg(err))
    }

    fn send_clipboard(self, hash: &str, mut builder: HttpResponseBuilder) -> HttpResponse {
        builder.content_type(Self::CONTENT_TYPE);

        if let Err(err) = self.0 {
            return builder.body(Self::format_err(hash, err));
        }

        let maybe_body = String::from_utf8(self.0.unwrap().unwrap().to_vec());
        if let Err(conv_err) = maybe_body {
            let body = json!({ "error": StoreError::InvalidUtf8(conv_err) }).to_string();
            return builder.body(body);
        }

        let body = maybe_body.unwrap();
        builder.body(body)
    }

    fn post_clipboard(self, hash: &str, mut builder: HttpResponseBuilder) -> HttpResponse {
        let body;

        if let Err(err) = self.0 {
            body = Self::format_err(hash, err);
        } else {
            body = format!("clipboard {hash} created and available at /api/drop/{hash}");
        }

        builder.content_type(Self::CONTENT_TYPE).body(body)
    }
}

impl DropResponseHttp for ResponseJson {
    const CONTENT_TYPE: &'static str = "application/json";

    fn format_err(hash: &str, err: StoreError) -> String {
        json!({
            "error": extract_error_msg(err),
            "clipboard": hash,
        })
        .to_string()
    }

    fn send_clipboard(self, hash: &str, mut builder: HttpResponseBuilder) -> HttpResponse {
        builder.content_type(Self::CONTENT_TYPE);

        if let Err(err) = self.0 {
            return builder.body(Self::format_err(hash, err));
        }

        let maybe_body = String::from_utf8(self.0.unwrap().unwrap().to_vec());
        if let Err(conv_err) = maybe_body {
            let body = serde_json::to_string(&StoreError::InvalidUtf8(conv_err)).unwrap();
            return builder.body(body);
        }

        let body = maybe_body.unwrap();
        builder.body(body)
    }

    fn post_clipboard(self, hash: &str, mut builder: HttpResponseBuilder) -> HttpResponse {
        let body;
        if let Err(err) = self.0 {
            body = Self::format_err(hash, err);
        } else {
            body = json!({
                "success": true,
                "clipboard": hash,
            })
            .to_string();
        }

        builder.content_type(Self::CONTENT_TYPE).body(body)
    }
}

pub const HEADER: &str = r#"<!DOCTYPE html><html><head><meta name=viewport content="width=device-width, initial-scale=1.0"><meta name=keywords content="actix-drop"><meta name=author content=@artnoi><meta charset=UTF-8><link href=https://artnoi.com/style.css rel=stylesheet><title>actix-drop</title></head><body><h1><a href="/">actix-drop</a></h1>"#;
pub const FOOTER: &str = r#"<footer><p><a href="https://github.com/artnoi43/actix-drop">Contribute on Github</a></p></footer></body></html>"#;

pub fn wrap_html(s: &str) -> String {
    format!("{}{}{}", HEADER, s, FOOTER)
}

fn extract_error_msg(err: StoreError) -> String {
    if let Some(public_err) = public_error(err) {
        return public_err.to_string();
    }

    return "private error".to_string();
}
