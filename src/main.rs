use std::time::Duration;

use actix_web::{middleware, web, App, HttpResponse, HttpServer};
use colored::Colorize;
use serde::Deserialize;
use sha2::{Digest, Sha256};

mod config; // actix-drop config, not extern crate `config`
mod resp;
mod store;

use crate::config::AppConfig;
use store::clipboard::Clipboard;
use store::data::Data;
use store::error::StoreError;
use store::tracker::Tracker;

// Load CSS at compile time
const CSS: &str = include_str!("../assets/style.css");

/// `ReqForm` is used to mirror `Clipboard`
/// so that our HTML form deserialization is straightforward.
/// `ReqForm` in JSON looks like this: `{"store": "mem", "data": "my_data"}`
/// while `Clipboard` looks like this: `{"mem": "my_data"}`
#[derive(Deserialize)]
struct ReqForm {
    store: String,
    data: Data,
}

impl Into<Clipboard> for ReqForm {
    fn into(self) -> Clipboard {
        Clipboard::new_with_data(&self.store, self.data)
    }
}

async fn landing<R: resp::DropResponseHttp>() -> HttpResponse {
    R::landing_page()
}

/// post_drop receives Clipboard from HTML form (sent by the form in landing_page) or JSON request,
/// and save text to file. The text will be hashed, and the first 4 hex-encoded string of the hash
/// will be used as filename as ID for the clipboard.
/// When a new clipboard is posted, post_drop sends a message via tx to register the expiry timer.
async fn post_drop<F, J, R>(
    tracker: web::Data<Tracker>,
    dur: web::Data<Duration>,
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
        return R::from(Err(err)).post_clipboard(HttpResponse::BadRequest(), "");
    }

    if clipboard.is_empty() {
        return R::from(Err(StoreError::Empty)).post_clipboard(HttpResponse::BadRequest(), "");
    }

    // hash is hex-coded string of SHA2 hash of clipboard.text.
    // hash will be truncated to string of length 4, and used as clipboard key.
    let mut hash = format!("{:x}", Sha256::digest(&clipboard));
    hash.truncate(4);

    if let Err(err) = Tracker::store_new_clipboard(
        tracker.into_inner(),
        &hash,
        clipboard,
        Duration::from(**dur),
    ) {
        eprintln!("error storing clipboard {}: {}", hash, err.to_string());

        let resp = R::from(Err(err));
        return resp.post_clipboard(HttpResponse::InternalServerError(), &hash);
    }

    R::from(Ok(None)).post_clipboard(HttpResponse::Ok(), &hash)
}

/// get_drop retrieves and returns the clipboard based on its hashed ID as per post_drop.
async fn get_drop<R: resp::DropResponseHttp>(
    tracker: web::Data<Tracker>,
    path: web::Path<String>,
) -> HttpResponse {
    let hash = path.into_inner();
    let tracker = tracker.into_inner();

    match tracker.get_clipboard(&hash) {
        Some(clipboard) => {
            R::from(Ok(Some(clipboard))).send_clipboard(HttpResponse::Ok(), &hash)
        }
        None => {
            R::from(Err(StoreError::NoSuch)).send_clipboard(HttpResponse::NotFound(), &hash)
        }
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
        .route("/drop", web::post().to(post_drop::<ReqForm, Clipboard, R>))
}

#[actix_web::main]
#[cfg(unix)]
async fn main() {
    let conf = AppConfig::init();
    println!(
        "\n{}\n{}\n",
        "Starting actix-drop: current configuration".yellow(),
        serde_json::to_string(&conf).unwrap()
    );

    // Ensure that ./${DIR} is a directory
    store::persist::assert_dir(conf.dir);

    let server = HttpServer::new(move || {
        App::new()
            .wrap(middleware::NormalizePath::new(
                middleware::TrailingSlash::Trim,
            ))
            .wrap(middleware::NormalizePath::new(
                middleware::TrailingSlash::MergeOnly,
            ))
            .app_data(web::Data::new(Duration::from_secs(
                conf.timeout.expect("timeout is None"),
            )))
            .app_data(web::Data::new(String::from(CSS)))
            .app_data(web::Data::new(Tracker::new()))
            .service(web::resource("/style.css").route(web::get().to(serve_css)))
            .service(routes::<resp::ResponseHtml>("/app"))
            .service(routes::<resp::ResponseJson>("/api"))
            .service(routes::<resp::ResponseText>("/txt"))
    });

    let http_addr = format!(
        "{}:{}",
        conf.http_addr.expect("http_addr is None"),
        conf.http_port.expect("http_port is None")
    );

    println!(
        "{} {}",
        "Listening on".yellow(),
        format!("http://{}", http_addr).cyan()
    );

    server
        .bind(http_addr)
        .expect(&"error binding server to address".red())
        .run()
        .await
        .expect(&"error running server".red());
}
