use actix_web::{get, post, web, App, HttpResponse, HttpServer};
use sha2::{Digest, Sha256};

mod data;
mod html;
mod persist;
mod store;

use store::Store;

// TODO: new struct or manually implement Deserialize
type ClipboardRequest = Store;

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
            store::MEM,
            store::PERSIST,
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
    // Extract clipboard from web::Either<web::Form, web::Json>
    let clipboard: ClipboardRequest = req.into_inner();
    if clipboard.is_empty() {
        return HttpResponse::BadRequest()
            .content_type("text/html")
            .body(html::wrap_html("<p>Error: blank clipboard sent</p>"));
    }

    // hash is hex-coded string of SHA2 hash of form.text.
    // hash will be truncated to string of length 4, and
    // the short stringa
    let mut hash = format!("{:x}", Sha256::digest(&clipboard));
    hash.truncate(4);

    match clipboard {
        Store::Persist(data) => {
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

    let addr = "127.0.0.1:3000";
    println!("actix-drop listening on {}...", addr);
    server
        .bind(addr)
        .expect("error binding server to address")
        .run()
        .await
        .expect("error running server");
}
