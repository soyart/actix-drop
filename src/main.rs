use actix_web::{get, post, web, App, HttpResponse, HttpServer};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::env;
use std::path::Path;

mod html;

// Default hard-coded storage directory.
const DIR: &str = "drop";

#[derive(Deserialize)]
struct Clipboard {
    text: String,
}

// Return HTML form for entering text to be saved
async fn landing_page() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html")
        .body(html::wrap_html(
            r#"<form action="/drop" method="post">
            <textarea id="textbox" name="text" rows="5" cols="32"></textarea><br>
            <button type="submit">Send</button>
            </form>"#,
        ))
}

// Receive Clipboard from HTML form sent by get_index, and save text to file.
// The text will be hashed, and the first 4 hex-encoded string of the hash
// will be used as filename as ID for the clipboard.
#[post("/drop")]
async fn post_drop(mut form: web::Form<Clipboard>) -> HttpResponse {
    if form.text.is_empty() {
        return HttpResponse::BadRequest()
            .content_type("text/html")
            .body(html::wrap_html("<p>Error: blank clipboard sent</p>"));
    }

    // hash is hex-coded string of SHA2 hash of form.text.
    // hash will be truncated to string of length 4, and
    // the short stringa
    let mut hash = format!("{:x}", Sha256::digest(&form.text));
    hash.truncate(4);

    if let Err(err) = write_clipboard(&hash, form.text.as_ref()) {
        eprintln!("write_file error: {}", err.to_string());

        return HttpResponse::InternalServerError()
            .content_type("text/html")
            .body(html::wrap_html("<p>Error: cannot save clipboard</p>"));
    }

    let mut dotdot: &str = "";
    if form.text.len() > 10 {
        form.text.truncate(10);
        dotdot = "..";
    }

    let body = format!(
        r#"<p>Clipboard <code>{0}</code>{1} with hash <code>{2}</code> created</p>
        <p>The clipboard is now available at path <a href="/drop/{2}"><code>/drop/{2}</code></a></p>"#,
        form.text, dotdot, hash,
    );

    HttpResponse::Created()
        .content_type("text/html")
        .body(html::wrap_html(&body))
}

// Retrive the clipboard based on its ID as per post_drop.
#[get("/drop/{id}")]
async fn get_drop(id: web::Path<String>) -> HttpResponse {
    match read_clipboard(id.clone().into()) {
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

fn write_clipboard<S>(name: S, content: &[u8]) -> std::io::Result<()>
where
    S: AsRef<Path>,
{
    let path = Path::new(DIR).join(name.as_ref());
    std::fs::write(path, content)
}

fn read_clipboard(id: web::Path<String>) -> std::io::Result<Vec<u8>> {
    let path = Path::new(DIR).join(id.as_ref());
    std::fs::read(path)
}

fn create_dir(dir: &str) -> std::io::Result<()> {
    std::fs::create_dir(dir)
}

fn check_dir(dst: &str) -> std::io::Result<bool> {
    let mut pwd = env::current_dir()?;
    pwd.push(dst);
    let metadata = std::fs::metadata(pwd)?;
    Ok(metadata.is_dir())
}

#[actix_web::main]
async fn main() {
    // Ensure that ./${DIR} is a directory
    match check_dir(DIR) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            create_dir(DIR).expect("failed to create storage directory");
        }

        Err(err) => {
            panic!(
                "failed to get working directory information: {}",
                err.to_string()
            );
        }

        Ok(false) => create_dir(DIR).expect("failed to create storage directory"),

        _ => {}
    }

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
