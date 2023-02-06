use actix_web::{get, post, web, App, HttpResponse, HttpServer};
use serde::Deserialize;
use sha2::{Digest, Sha256};

mod html;
mod store;

use store::data::Data;
use store::error::StoreError;
use store::Store;

#[derive(Deserialize)] // eg: {"store": "mem", "persist": "my_data"}
struct ReqForm {
    store: String,
    data: Data,
}

impl Into<Store> for ReqForm {
    fn into(self) -> Store {
        Store::new_with_data(&self.store, self.data)
    }
}

type ReqJson = store::Store; // eg: {"mem" = "my_data" }

/// Return HTML form for entering text to be saved
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

/// post_drop receives Clipboard from HTML form (sent by the form in landing_page) or JSON request,
/// and save text to file. The text will be hashed, and the first 4 hex-encoded string of the hash
/// will be used as filename as ID for the clipboard.
#[post("/drop")]
async fn post_drop<'a>(req: web::Either<web::Form<ReqForm>, web::Json<ReqJson>>) -> HttpResponse {
    // Extract clipboard from web::Either<web::Form, web::Json>
    let clipboard = match req {
        web::Either::Left(web::Form(req_form)) => req_form.into(),
        web::Either::Right(web::Json(req_json)) => req_json,
    };

    if let Err(err) = clipboard.is_implemented() {
        return HttpResponse::BadRequest()
            .content_type("text/html")
            .body(html::wrap_html(&format!(
                "<p>Error: bad clipboard store: {}</p>",
                err.to_string(),
            )));
    }

    if clipboard.is_empty() {
        return HttpResponse::BadRequest()
            .content_type("text/html")
            .body(html::wrap_html("<p>Error: blank clipboard sent</p>"));
    }

    // hash is hex-coded string of SHA2 hash of form.text.
    // hash will be truncated to string of length 4, and used as clipboard key.
    let mut hash = format!("{:x}", Sha256::digest(&clipboard));
    hash.truncate(4);

    if clipboard.save_clipboard(&hash).is_err() {
        return HttpResponse::InternalServerError()
            .content_type("text/html")
            .body(html::wrap_html("<p>Error: cannot save clipboard</p>"));
    }

    let body = format!(
        r#"<p>Clipboard with hash <code>{1}</code> created</p>
        <p>The clipboard is now available at path <a href="/drop/{0}/{1}"><code>/drop/{0}/{1}</code></a></p>"#,
        clipboard.key(),
        hash,
    );

    HttpResponse::Created()
        .content_type("text/html")
        .body(html::wrap_html(&body))
}

/// get_drop retrieves and returns the clipboard based on its storage and ID as per post_drop.
#[get("/drop/{store}/{id}")]
async fn get_drop(path: web::Path<(String, String)>) -> HttpResponse {
    let (store, id) = path.into_inner();
    let mut store = Store::new(&store);

    match store.read_clipboard(&id) {
        Err(StoreError::Bug(err)) => {
            eprintln!("actix-drop bug: {}", err.to_string());
            let body = format!(
                "Error: found unexpected error for clipboard: <code>{}</code>",
                id
            );

            return HttpResponse::InternalServerError()
                .content_type("text/html")
                .body(html::wrap_html(&body));
        }

        Err(err) => {
            eprintln!("read_clipboard error: {}", err.to_string());
            let body = format!("Error: no such clipboard: <code>{}</code>", id);

            return HttpResponse::NotFound()
                .content_type("text/html")
                .body(html::wrap_html(&body));
        }

        Ok(()) => {
            let text = String::from_utf8(store.to_vec());
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
    store::persist::assert_dir();

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
