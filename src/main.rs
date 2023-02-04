use actix_web::{get, post, web, App, HttpResponse, HttpServer};
use serde::Deserialize;
use sha2::{Digest, Sha256};

mod html;
mod persist;

const MEM: &str = "MEM";
const PERSIST: &str = "PERSIST";

// enum Store specifies which type of storage to use
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Store<T>
where
    T: AsRef<[u8]>,
{
    Mem(T),
    Persist(T),
}

impl<T> std::ops::Deref for Store<T>
where
    T: AsRef<[u8]>,
{
    type Target = T;

    fn deref(self: &Self) -> &Self::Target {
        match self {
            Self::Mem(t) => return &t,
            Self::Persist(t) => return &t,
        }
    }
}

impl<T> AsRef<[u8]> for Store<T>
where
    T: AsRef<[u8]>,
{
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Mem(t) => return t.as_ref(),
            Self::Persist(t) => return t.as_ref(),
        }
    }
}

impl<T> std::fmt::Display for Store<T>
where
    T: AsRef<[u8]>,
{
    fn fmt(self: &Self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let key = match self {
            Self::Persist(_) => PERSIST,
            Self::Mem(_) => MEM,
        };

        let val = self.as_ref();
        let s = std::str::from_utf8(val);

        if let Ok(string) = s {
            write!(formatter, r#""{}":"{}""#, key, string)
        } else {
            write!(formatter, r#""{}":"{:?}"#, key, val)
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_display_store() {
        use super::Store;
        let mem_str = Store::Mem("foo");
        assert_eq!(r#""MEM":"foo""#, format!("{}", mem_str));

        let persist_bin = Store::Persist(vec![14, 16, 200]);
        assert_eq!(r#""PERSIST":"[14, 16, 200]"#, format!("{}", persist_bin));

        // Valid UTF-8 byte array should be formatted as string
        let mem_str_vec = Store::Mem("foo".bytes().collect::<Vec<u8>>());
        assert_eq!(r#""MEM":"foo""#, format!("{}", mem_str_vec));
    }
}

// TODO: new struct or manually implement Deserialize
#[derive(Deserialize)]
struct ClipboardRequest {
    data: String,
    store: String,
}

// Return HTML form for entering text to be saved
async fn landing_page() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html")
        .body(html::wrap_html(&format!(
            r#"<form action="/drop" method="post">
            <textarea id="textbox" name="data" rows="5" cols="32"></textarea><br>
            <select id="selection box" name="store">
                <option value="{}">In-memory database</option>
                <option value="{}">Persist to file</option>
            </select>
            <button type="submit">Send</button>
            </form>"#,
            MEM, PERSIST,
        )))
}

// Receive Clipboard from HTML form sent by get_index, and save text to file.
// The text will be hashed, and the first 4 hex-encoded string of the hash
// will be used as filename as ID for the clipboard.
// It can handle both HTML form and JSON request.
#[post("/drop")]
async fn post_drop<'a>(
    req: web::Either<web::Form<ClipboardRequest>, web::Json<ClipboardRequest>>,
) -> HttpResponse {
    let form = req.into_inner();
    let data: Store<&[u8]>;

    match form.store.as_ref() {
        PERSIST => {
            data = Store::Persist(form.data.as_ref());
        }

        _ => {
            data = Store::Mem(form.data.as_ref());
        }
    }

    if data.is_empty() {
        return HttpResponse::BadRequest()
            .content_type("text/html")
            .body(html::wrap_html("<p>Error: blank clipboard sent</p>"));
    }

    // hash is hex-coded string of SHA2 hash of form.text.
    // hash will be truncated to string of length 4, and
    // the short stringa
    let data = &data;
    let mut hash = format!("{:x}", Sha256::digest(data));
    hash.truncate(4);

    match data {
        Store::Persist(_) => {
            if let Err(err) = persist::write_clipboard_file(&hash, data.as_ref()) {
                eprintln!("write_file error: {}", err.to_string());

                return HttpResponse::InternalServerError()
                    .content_type("text/html")
                    .body(html::wrap_html("<p>Error: cannot save clipboard</p>"));
            }
        }

        // Send Store::Mem to another thread
        Store::Mem(_) => {
            return HttpResponse::InternalServerError()
                .content_type("text/html")
                .body(html::wrap_html(
                    "<p>Error: in-memory store or bytes clipboard not implemented</p>",
                ));
        }
    }

    let body = format!(
        r#"<p>Clipboard with hash <code>{0}</code> created</p>
        <p>The clipboard is now available at path <a href="/drop/{0}"><code>/drop/{0}</code></a></p>"#,
        hash,
    );

    HttpResponse::Created()
        .content_type("text/html")
        .body(html::wrap_html(&body))
}

// Retrive the clipboard based on its ID as per post_drop.
#[get("/drop/{id}")]
async fn get_drop(id: web::Path<String>) -> HttpResponse {
    match persist::read_clipboard_file::<std::path::PathBuf>(id.clone().into()) {
        Err(err) => {
            eprintln!("read_clipboard error: {}", err.to_string());

            let body = format!("Error: no such clipboard: <code>{}</code>", id);
            return HttpResponse::NotFound()
                .content_type("text/html")
                .body(html::wrap_html(&body));
        }

        Ok(clipboard) => {
            let text = String::from_utf8(clipboard);
            if text.is_err() {
                return HttpResponse::InternalServerError()
                    .content_type("text/html")
                    .body(html::wrap_html("Error: clipboard is non UTF-8"));
            }

            let body = format!(
                r#"<p>Clipboard <code>{}</code>:</p>
                <pre><code>{}</code></pre>"#,
                id,
                text.unwrap(),
            );

            return HttpResponse::Ok()
                .content_type("text/html")
                .body(html::wrap_html(&body));
        }
    }
}

async fn serve_css() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/css")
        .body(html::STYLE)
}

#[actix_web::main]
async fn main() {
    // Ensure that ./${DIR} is a directory
    persist::assert_dir();

    let server = HttpServer::new(|| {
        App::new()
            .route("/", web::get().to(landing_page))
            .route("/drop", web::get().to(landing_page))
            .route("/style.css", web::get().to(serve_css))
            .service(get_drop)
            .service(post_drop)
    });

    let addr = "http://127.0.0.1:3000";
    println!("actix-drop listening on {}...", addr);
    server
        .bind(addr)
        .expect("error binding server to address")
        .run()
        .await
        .expect("error running server");
}
