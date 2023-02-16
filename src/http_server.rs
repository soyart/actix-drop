use std::time::Duration;

use actix_web::{web, HttpResponse};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::resp::http_resp;
use crate::store::clipboard::Clipboard;
use crate::store::data::Data;
use crate::store::error::StoreError;
use crate::store::tracker::Tracker;

// Load CSS at compile time
pub const CSS: &str = include_str!("../assets/style.css");

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

async fn landing<R: http_resp::DropResponseHttp>() -> HttpResponse {
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
    R: http_resp::DropResponseHttp,
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
async fn get_drop<R>(tracker: web::Data<Tracker>, path: web::Path<String>) -> HttpResponse
where
    R: http_resp::DropResponseHttp,
{
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

// Serve CSS serves the CSS from actix-web shared immutable state `web::Data`
pub async fn serve_css(css: web::Data<String>) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/css")
        .body(css.into_inner().as_ref().clone())
}

/// routes setup different routes for each R with prefix `prefix`.
/// TODO: Test routes availability, and remove duplicate routes at "" and "/"
pub fn routes<R>(prefix: &str) -> actix_web::Scope
where
    R: http_resp::DropResponseHttp + 'static,
{
    web::scope(prefix)
        .route("", web::get().to(landing::<R>))
        .route("/", web::get().to(landing::<R>))
        .route("/drop/{id}", web::get().to(get_drop::<R>))
        .route("/drop", web::post().to(post_drop::<ReqForm, Clipboard, R>))
}

#[cfg(test)]
mod http_server_tests {
    use actix_web::{http::header::ContentType, middleware, test, App};

    use super::routes;
    use crate::resp::http_resp::{ResponseHtml, ResponseJson, ResponseText};

    #[rustfmt::skip]
        macro_rules! setup_app {
            () => {
                test::init_service(
                    App::new()
                        .wrap(middleware::NormalizePath::new(
                            middleware::TrailingSlash::Trim,
                        ))
                        .service(routes::<ResponseHtml>("/app"))
                        .service(routes::<ResponseJson>("/api"))
                        .service(routes::<ResponseText>("/txt")),
                )
                .await
            };
        }

    #[actix_web::test]
    async fn test_default_routes() {
        let app = setup_app!();

        let reqs = vec![
            ("/app", ContentType::html()),
            ("/api", ContentType::json()),
            ("/txt", ContentType::plaintext()),
            ("/app/", ContentType::html()),
            ("/api/", ContentType::json()),
            ("/txt/", ContentType::plaintext()),
        ]
        .into_iter()
        .map(|(endpoint, content_type)| {
            test::TestRequest::get()
                .uri(endpoint)
                .insert_header(content_type.clone())
                .to_request()
        });

        for req in reqs {
            let resp = test::call_service(&app, req).await;
            println!("req: {:?} {}", resp.request(), resp.status());

            assert!(resp.status().is_success());
        }
    }
}
