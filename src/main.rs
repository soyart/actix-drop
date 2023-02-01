use actix_web::{get, post, web, App, HttpResponse, HttpServer};
use serde::Deserialize;
use sha2::{Digest, Sha256};

mod html;
mod persist;

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
    T: std::fmt::Display + AsRef<[u8]>,
{
    fn fmt(self: &Self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mem(_) => write!(formatter, "mem"),
            Self::Persist(_) => write!(formatter, "persist"),
        }
    }
}

type Clipboard = Vec<u8>;

// TODO: new struct or manually implement Deserialize
#[derive(Deserialize)]
struct ClipboardRequest {
    data: String,
    store: Store<Clipboard>,
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
            "mem", "persist",
        )))
}

// Receive Clipboard from HTML form sent by get_index, and save text to file.
// The text will be hashed, and the first 4 hex-encoded string of the hash
// will be used as filename as ID for the clipboard.
// It can handle both HTML form and JSON request.
#[post("/drop")]
async fn post_drop(
    req: web::Either<web::Form<ClipboardRequest>, web::Json<ClipboardRequest>>,
) -> HttpResponse {
    let form = req.into_inner();
    let data: Store<&[u8]>;

    match form.store {
        Store::Persist(_) => {
            data = Store::Persist(form.data.as_ref());
        }

        _ => {
            return HttpResponse::InternalServerError()
                .content_type("text/html")
                .body(html::wrap_html(
                    "<p>Error: in-memory store or bytes clipboard not implemented</p>",
                ));
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

    match form.store {
        Store::Persist(_) => {
            if let Err(err) = persist::write_clipboard_file(&hash, data.as_ref()) {
                eprintln!("write_file error: {}", err.to_string());

                return HttpResponse::InternalServerError()
                    .content_type("text/html")
                    .body(html::wrap_html("<p>Error: cannot save clipboard</p>"));
            }
        }

        Store::Mem(_) => {
            return HttpResponse::InternalServerError()
                .content_type("text/html")
                .body(html::wrap_html(
                    "<p>Error: in-memory store not implemented</p>",
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
            .service(post_drop)
            .service(get_drop)
    });

    println!("actix-drop listening on http://localhost:3000...");
    server
        .bind("127.0.0.1:3000")
        .expect("error binding server to address")
        .run()
        .await
        .expect("error running server");
}
