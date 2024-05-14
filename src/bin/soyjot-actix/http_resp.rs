use actix_web::{HttpResponse, HttpResponseBuilder};
use serde_json::json;

use soyjot::html::{self, wrap_html};
use soyjot::store::clipboard::{self, Clipboard};
use soyjot::store::error::{public_error, StoreError};
use soyjot::{para, tag_html};

/// DropResult represents clipboard or error from http_server
/// The clipboard is wrapped in `Option` because when posting clipboard,
/// the response contains to clipboard (None) but yet there's no error.
type DropResult = Result<Option<Clipboard>, StoreError>;

/// DropResponseHttp is a trait representing actix-drop HTTP response.
pub trait DropResponseHttp: From<(HttpResponseBuilder, DropResult)> {
    // HTTP header Content-Type
    const CONTENT_TYPE: &'static str;

    /// landing_page is the default endpoint for R.
    /// It should return some kind of OK status and text,
    /// and for HTML resposnes, it should offer some kind of user input.
    fn landing_page() -> HttpResponse;

    /// format_err formats StoreError
    fn format_err(hash: &str, err: StoreError) -> String;

    /// send_clipboard returns the response with the clipboard content
    /// self should be Ok(Some(_)), since we are sending the clipboard to clients.
    fn send_clipboard(self, hash: &str) -> HttpResponse;

    /// post_clipboard returns the response when clipboard is posted to actix-drop
    /// self should be Ok(None), since we are not sending just the acknowledgement.
    fn post_clipboard(self, hash: &str) -> HttpResponse;
}

/// ResponseHtml implements DropResponseHttp for HTML responses
pub struct ResponseHtml(HttpResponseBuilder, DropResult);
/// ResponseHtml implements DropResponseHttp for plain text responses
pub struct ResponseText(HttpResponseBuilder, DropResult);
/// ResponseHtml implements DropResponseHttp for JSON text responses
pub struct ResponseJson(HttpResponseBuilder, DropResult);

macro_rules! impl_from_drop_result {
    ( $( $t: ident );+ ) => {
            $(
                impl From<(HttpResponseBuilder, DropResult)> for $t {
                    fn from(result: (HttpResponseBuilder, DropResult)) -> $t {
                        $t(result.0, result.1)
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
                r#"<form action="/app/drop" method="post">
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

    fn send_clipboard(mut self, hash: &str) -> HttpResponse {
        let body = match self.1 {
            Err(err) => Self::format_err(hash, err),

            Ok(Some(ref clipboard)) => match String::from_utf8(clipboard.to_vec()) {
                Ok(clip_string) => format!(
                    r#"<p>Clipboard <code>{hash}</code>:</p>
                    <pre><code>{clip_string}</code></pre>"#,
                ),

                Err(err) => Self::format_err(hash, StoreError::InvalidUtf8(err)),
            },

            Ok(None) => panic!("Ok(None) in match arm"),
        };

        self.0
            .content_type(Self::CONTENT_TYPE)
            .body(html::wrap_html(&body))
    }

    fn post_clipboard(mut self, hash: &str) -> HttpResponse {
        let body = match self.1 {
            Err(err) => {
                format!(
                    "<p>Error saving clipboard {hash}: {}</p>",
                    extract_error_msg(err)
                )
            }

            Ok(None) => {
                format!(
                    r#"<p>Clipboard with hash <code>{hash}</code> created</p>
                        <p>The clipboard is now available at path <a href="/app/drop/{hash}"><code>/app/drop/{hash}</code></a></p>"#
                )
            }

            Ok(Some(_)) => panic!("Ok(Some) in match arm"),
        };

        self.0
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

    fn send_clipboard(mut self, hash: &str) -> HttpResponse {
        let body = match self.1 {
            Err(err) => Self::format_err(hash, err),
            Ok(Some(clipboard)) => match String::from_utf8(clipboard.to_vec()) {
                Ok(clip_string) => clip_string,
                Err(err) => Self::format_err(hash, StoreError::InvalidUtf8(err)),
            },

            Ok(None) => panic!("Ok(None) in match arm"),
        };

        self.0.content_type(Self::CONTENT_TYPE).body(body)
    }

    fn post_clipboard(mut self, hash: &str) -> HttpResponse {
        let body = match self.1 {
            Err(err) => Self::format_err(hash, err),
            Ok(None) => {
                format!("clipboard {hash} created and available at /api/drop/{hash}")
            }
            Ok(Some(_)) => panic!("Ok(Some) in match arm"),
        };

        self.0.content_type(Self::CONTENT_TYPE).body(body)
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

    fn send_clipboard(mut self, hash: &str) -> HttpResponse {
        let body = match self.1 {
            Err(err) => Self::format_err(hash, err),
            Ok(Some(clipboard)) => match String::from_utf8(clipboard.to_vec()) {
                Ok(clip_string) => clip_string,
                Err(err) => Self::format_err(hash, StoreError::InvalidUtf8(err)),
            },

            Ok(None) => panic!("Ok(None) in match arm"),
        };

        self.0.content_type(Self::CONTENT_TYPE).body(body)
    }

    fn post_clipboard(mut self, hash: &str) -> HttpResponse {
        let body = match self.1 {
            Err(err) => Self::format_err(hash, err),
            Ok(None) => json!({
                "clipboard": hash,
            })
            .to_string(),

            Ok(Some(_)) => panic!("Ok(Some) in match arm"),
        };

        self.0.content_type(Self::CONTENT_TYPE).body(body)
    }
}

pub fn extract_error_msg(err: StoreError) -> String {
    public_error(err)
        .unwrap_or_else(|| StoreError::Bug("private error".to_string()))
        .to_string()
}
