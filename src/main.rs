use actix_web::{get, web, App, HttpResponse, HttpServer};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::sync::{mpsc, Arc, Mutex};

mod html;
mod store;

use store::clipboard::Clipboard;
use store::data::Data;
use store::error::StoreError;
use store::tracker;

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
        .body(html::wrap_html(&format!(
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
    req: web::Either<web::Form<F>, web::Json<J>>,
    tx: web::Data<mpsc::Sender<(String, Clipboard)>>,
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

    // hash is hex-coded string of SHA2 hash of clipboard.text.
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

    let tx = tx.into_inner();
    if let Err(err) = tx.send((hash.to_string(), clipboard)) {
        panic!("failed to send to tx: {}", err.to_string());
    };

    HttpResponse::Created()
        .content_type("text/html")
        .body(html::wrap_html(&body))
}

/// get_drop retrieves and returns the clipboard based on its storage and ID as per post_drop.
#[get("/drop/{store}/{id}")]
async fn get_drop(path: web::Path<(String, String)>) -> HttpResponse {
    let (store, id) = path.into_inner();
    let mut store = Clipboard::new(&store);

    match store.read_clipboard(&id) {
        Err(err) => {
            let body;
            match err {
                StoreError::Bug(bug) => {
                    eprintln!("actix-drop bug: {}", bug.to_string());
                    body = format!(
                        "Error: found unexpected error for clipboard: <code>{}</code>",
                        id
                    );
                }
                _ => {
                    eprintln!("read_clipboard error: {}", err.to_string());
                    body = format!("Error: no such clipboard: <code>{}</code>", id);
                }
            }

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

    let (tx, rx) = mpsc::channel::<(String, Clipboard)>();

    let tracker = Arc::new(Mutex::new(tracker::Tracker::new()));
    let also_tracker = tracker.clone();

    // This thread sleeps for dur and then checks if any
    // item in tracker has expired. If so, it removes it from tracker
    let dur = std::time::Duration::from_secs(30);
    std::thread::spawn(move || {
        tracker::clear_expired_clipboards(tracker, dur);
    });

    // This thread loops forever and adds new item to tracker
    std::thread::spawn(|| tracker::loop_add_tracker(rx, also_tracker));

    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(tx.clone()))
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
