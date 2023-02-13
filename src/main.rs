use std::time::Duration;

use actix_web::{get, web, App, HttpResponse, HttpServer};
use serde::Deserialize;
use sha2::{Digest, Sha256};

mod resp;
mod store;

use store::clipboard::Clipboard;
use store::data::Data;
use store::error;
use store::tracker::{countdown_remove, Tracker};

const CSS: &str = include_str!("../assets/style.css");

#[derive(Deserialize)] // eg: {"store": "mem", "persist": "my_data"}
struct ReqForm {
    store: String,
    data: Data,
}

impl Into<Clipboard> for ReqForm {
    fn into(self) -> Clipboard {
        Clipboard::new_with_data(&self.store, self.data)
    }
}

type ReqJson = Clipboard; // eg: {"mem" = "my_data" }

/// Return HTML form for entering text to be saved
async fn landing_page() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html")
        .body(resp::wrap_html(&format!(
            r#"<form action="/drop" method="post">
            <textarea id="textbox" name="data" rows="5" cols="32"></textarea><br>
            <select id="selection box" name="store">
                <option value="{}">In-memory database</option>
                <option value="{}">Persist to file</option>
            </select>
            <button type="submit">Send</button>
            </form>"#,
            store::clipboard::MEM,
            store::clipboard::PERSIST,
        )))
}

/// post_drop receives Clipboard from HTML form (sent by the form in landing_page) or JSON request,
/// and save text to file. The text will be hashed, and the first 4 hex-encoded string of the hash
/// will be used as filename as ID for the clipboard.
/// When a new clipboard is posted, post_drop sends a message via tx to register the expiry timer.
async fn post_drop<F, J>(
    tracker: web::Data<Tracker>,
    req: web::Either<web::Form<F>, web::Json<J>>,
) -> HttpResponse
where
    F: Into<Clipboard>,
    J: Into<Clipboard>,
{
    let clipboard = match req {
        web::Either::Left(web::Form(form)) => form.into(),
        web::Either::Right(web::Json(json)) => json.into(),
    };

    if let Err(err) = clipboard.is_implemented() {
        return HttpResponse::BadRequest()
            .content_type("text/html")
            .body(resp::wrap_html(&format!(
                "<p>Error: bad clipboard store: {}</p>",
                err.to_string(),
            )));
    }

    if clipboard.is_empty() {
        return HttpResponse::BadRequest()
            .content_type("text/html")
            .body(resp::wrap_html("<p>Error: blank clipboard sent</p>"));
    }

    // hash is hex-coded string of SHA2 hash of clipboard.text.
    // hash will be truncated to string of length 4, and used as clipboard key.
    let mut hash = format!("{:x}", Sha256::digest(&clipboard));
    hash.truncate(4);

    let tracker = tracker.into_inner();
    if let Err(err) = tracker.store_new_clipboard(&hash, clipboard) {
        eprintln!("error storing clipboard {}: {}", hash, err.to_string());
        return HttpResponse::InternalServerError()
            .content_type("text/html")
            .body(resp::wrap_html("<p>Error: cannot save clipboard</p>"));
    }

    actix_rt::spawn(countdown_remove(
        tracker,
        hash.clone(),
        Duration::from_secs(10),
    ));

    let body = format!(
        r#"<p>Clipboard with hash <code>{hash}</code> created</p>
        <p>The clipboard is now available at path <a href="/drop/{hash}/"><code>/drop/{hash}/</code></a></p>"#,
    );

    HttpResponse::Created()
        .content_type("text/html")
        .body(resp::wrap_html(&body))
}

/// get_drop retrieves and returns the clipboard based on its hashed ID as per post_drop.
#[get("/drop/{id}/")]
async fn get_drop(tracker: web::Data<Tracker>, path: web::Path<String>) -> HttpResponse {
    let id = path.into_inner();
    let tracker = tracker.into_inner();

    let body;
    match tracker.get_clipboard(&id) {
        Some(clipboard) => {
            let text = String::from_utf8(clipboard.to_vec());
            if text.is_err() {
                return HttpResponse::InternalServerError()
                    .content_type("text/html")
                    .body(resp::wrap_html("Error: clipboard is non UTF-8"));
            }

            body = format!(
                r#"<p>Clipboard <code>{}</code>:</p>
                <pre><code>{}</code></pre>"#,
                id,
                text.unwrap(),
            );

            HttpResponse::Ok()
        }

        None => {
            body = format!("Error: no such clipboard: <code>{}</code>", id);
            HttpResponse::NotFound()
        }
    }
    .content_type("text/html")
    .body(resp::wrap_html(&body))
}

async fn serve_css(css: web::Data<String>) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/css")
        .body(css.into_inner().as_ref().clone())
}

#[actix_web::main]
#[cfg(unix)]
async fn main() {
    // Ensure that ./${DIR} is a directory
    store::persist::assert_dir();

    // This thread sleeps for dur and then checks if any
    // item in tracker has expired. If so, it removes it from tracker
    // TODO: Use this default value
    let dur = std::time::Duration::from_secs(30);

    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(dur))
            .app_data(web::Data::new(String::from(CSS)))
            .app_data(web::Data::new(Tracker::new()))
            .route("/", web::get().to(landing_page))
            .route("/style.css", web::get().to(serve_css))
            .route("/drop", web::get().to(landing_page))
            .route("/drop", web::post().to(post_drop::<ReqForm, ReqJson>))
            .service(get_drop)
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
