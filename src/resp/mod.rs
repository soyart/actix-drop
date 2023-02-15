pub mod html;

use actix_web::{HttpResponse, HttpResponseBuilder};
use serde_json::json;

use crate::store::clipboard::{self, Clipboard};
use crate::store::error::{public_error, StoreError};
use crate::{para, tag_html};
use html::wrap_html;

type DropResult = Result<Option<Clipboard>, StoreError>;

pub trait DropResponseHttp: From<DropResult> {
    const CONTENT_TYPE: &'static str;

    fn landing_page() -> HttpResponse;
    fn format_err(hash: &str, err: StoreError) -> String;
    fn send_clipboard(self, builder: HttpResponseBuilder, hash: &str) -> HttpResponse;
    fn post_clipboard(self, builder: HttpResponseBuilder, hash: &str) -> HttpResponse;
}

pub struct ResponseHtml(DropResult);
pub struct ResponseText(DropResult);
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

// Impl From<DropResult> for ResponseHtml, ResponsePlain, ResponseJson
impl_from_drop_result!(ResponseHtml; ResponseText; ResponseJson);

impl DropResponseHttp for ResponseHtml {
    const CONTENT_TYPE: &'static str = "text/html";

    fn landing_page() -> HttpResponse {
        HttpResponse::Ok()
            .content_type("text/html")
            .body(wrap_html(&format!(
                r#"<form action="/drop" method="post">
            <textarea id="textbox" name="data" rows="5" cols="32"></textarea><br>
            <select id="selection box" name="store">
                <option value="{}">In-memory database</option>
                <option value="{}">Persist to file</option>
            </select>
            <button type="submit">Send</button>
            </form>"#,
                clipboard::MEM,
                clipboard::PERSIST,
            )))
    }

    fn format_err(hash: &str, err: StoreError) -> String {
        format!(
            "<p>Error for clipboard {hash}: {}</p>",
            extract_error_msg(err)
        )
    }

    fn send_clipboard(self, mut builder: HttpResponseBuilder, hash: &str) -> HttpResponse {
        builder.content_type(Self::CONTENT_TYPE);

        if let Err(err) = self.0 {
            let body = html::wrap_html(&Self::format_err(hash, err));
            return builder.body(body);
        }

        let maybe_body = String::from_utf8(self.0.unwrap().unwrap().to_vec());
        if let Err(conv_err) = maybe_body {
            let body =
                html::wrap_html(&format!("error: {:?}", StoreError::InvalidUtf8(conv_err)));
            return builder.body(body);
        }

        let body = format!(
            r#"<p>Clipboard <code>{hash}</code>:</p>
            <pre><code>{}</code></pre>"#,
            maybe_body.unwrap(),
        );

        builder.body(html::wrap_html(&body))
    }

    fn post_clipboard(self, mut builder: HttpResponseBuilder, hash: &str) -> HttpResponse {
        let body;

        if let Err(err) = self.0 {
            body = format!(
                "<p>Error saving clipboard {hash}: {}</p>",
                extract_error_msg(err)
            );
        } else {
            body = format!(
                r#"<p>Clipboard with hash <code>{hash}</code> created</p>
                <p>The clipboard is now available at path <a href="/app/drop/{hash}"><code>/app/drop/{hash}</code></a></p>"#
            );
        }

        builder
            .content_type(Self::CONTENT_TYPE)
            .body(html::wrap_html(&body))
    }
}

impl DropResponseHttp for ResponseText {
    const CONTENT_TYPE: &'static str = "text/plain; charset=utf-8";

    fn landing_page() -> HttpResponse {
        HttpResponse::Ok()
            .content_type(Self::CONTENT_TYPE)
            .body(para!("actix-drop: ok"))
    }

    fn format_err(hash: &str, err: StoreError) -> String {
        format!("error for clipboard {hash}: {}", extract_error_msg(err))
    }

    fn send_clipboard(self, mut builder: HttpResponseBuilder, hash: &str) -> HttpResponse {
        builder.content_type(Self::CONTENT_TYPE);

        if let Err(err) = self.0 {
            return builder.body(Self::format_err(hash, err));
        }

        let maybe_body = String::from_utf8(self.0.unwrap().unwrap().to_vec());
        if let Err(conv_err) = maybe_body {
            return builder.body(Self::format_err(hash, StoreError::InvalidUtf8(conv_err)));
        }

        let body = maybe_body.unwrap();
        builder.body(body)
    }

    fn post_clipboard(self, mut builder: HttpResponseBuilder, hash: &str) -> HttpResponse {
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

    fn landing_page() -> HttpResponse {
        HttpResponse::Ok()
            .content_type(Self::CONTENT_TYPE)
            .body("actix-drop: ok")
    }

    fn format_err(hash: &str, err: StoreError) -> String {
        json!({
            "error": extract_error_msg(err),
            "clipboard": hash,
        })
        .to_string()
    }

    fn send_clipboard(self, mut builder: HttpResponseBuilder, hash: &str) -> HttpResponse {
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

    fn post_clipboard(self, mut builder: HttpResponseBuilder, hash: &str) -> HttpResponse {
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

pub fn extract_error_msg(err: StoreError) -> String {
    if let Some(public_err) = public_error(err) {
        return public_err.to_string();
    }

    return "private error".to_string();
}
