use std::time::Duration;

use actix_web::{middleware, web, App, HttpResponse, HttpServer};
use serde::Deserialize;
use sha2::{Digest, Sha256};

mod resp;
mod store;

use store::clipboard::Clipboard;
use store::data::Data;
use store::error::StoreError;
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

async fn landing<R: resp::DropResponseHttp>() -> HttpResponse {
    R::landing_page()
}

/// post_drop receives Clipboard from HTML form (sent by the form in landing_page) or JSON request,
/// and save text to file. The text will be hashed, and the first 4 hex-encoded string of the hash
/// will be used as filename as ID for the clipboard.
/// When a new clipboard is posted, post_drop sends a message via tx to register the expiry timer.
async fn post_drop<F, J, R>(
    tracker: web::Data<Tracker>,
    req: web::Either<web::Form<F>, web::Json<J>>,
) -> HttpResponse
where
    F: Into<Clipboard>,
    J: Into<Clipboard>,
    R: resp::DropResponseHttp,
{
    let clipboard = match req {
        web::Either::Left(web::Form(form)) => form.into(),
        web::Either::Right(web::Json(json)) => json.into(),
    };

    if let Err(err) = clipboard.is_implemented() {
        return R::from(Err(err)).post_clipboard("", HttpResponse::BadRequest());
    }

    if clipboard.is_empty() {
        return R::from(Err(StoreError::Empty)).post_clipboard("", HttpResponse::BadRequest());
    }

    // hash is hex-coded string of SHA2 hash of clipboard.text.
    // hash will be truncated to string of length 4, and used as clipboard key.
    let mut hash = format!("{:x}", Sha256::digest(&clipboard));
    hash.truncate(4);

    let tracker = tracker.into_inner();
    if let Err(err) = tracker.store_new_clipboard(&hash, clipboard) {
        eprintln!("error storing clipboard {}: {}", hash, err.to_string());

        let resp = R::from(Err(err));
        return resp.post_clipboard(&hash, HttpResponse::InternalServerError());
    }

    actix_rt::spawn(countdown_remove(
        tracker,
        hash.clone(),
        Duration::from_secs(10),
    ));

    R::from(Ok(None)).post_clipboard(&hash, HttpResponse::Ok())
}

/// get_drop retrieves and returns the clipboard based on its hashed ID as per post_drop.
async fn get_drop<R: resp::DropResponseHttp>(
    tracker: web::Data<Tracker>,
    path: web::Path<String>,
) -> HttpResponse {
    let id = path.into_inner();
    let tracker = tracker.into_inner();

    match tracker.get_clipboard(&id) {
        Some(clipboard) => R::from(Ok(Some(clipboard))).send_clipboard(&id, HttpResponse::Ok()),
        None => R::from(Err(StoreError::NoSuch)).send_clipboard(&id, HttpResponse::NotFound()),
    }
}

async fn serve_css(css: web::Data<String>) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/css")
        .body(css.into_inner().as_ref().clone())
}

fn routes<R: resp::DropResponseHttp + 'static>(prefix: &str) -> actix_web::Scope {
    web::scope(prefix)
        .route("/", web::get().to(landing::<R>))
        .route("/drop/{id}", web::get().to(get_drop::<R>))
        .route("/drop", web::post().to(post_drop::<ReqForm, ReqJson, R>))
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
            .wrap(middleware::NormalizePath::new(
                // Path "/foo/" becomes "/foo"
                middleware::TrailingSlash::Trim,
            ))
            .wrap(middleware::NormalizePath::new(
                // Path "/foo//bar" becomes "/foo/bar"
                middleware::TrailingSlash::MergeOnly,
            ))
            .service(web::resource("/style.css").route(web::get().to(serve_css)))
            .service(routes::<resp::ResponseHtml>("/app"))
            .service(routes::<resp::ResponsePlain>("/text"))
            .service(routes::<resp::ResponseJson>("/api"))
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
