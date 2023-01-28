use actix_web::{get, post, web, App, HttpResponse, HttpServer};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::env;
use std::path::Path;

#[derive(Deserialize)]
struct Clipboard {
    text: String,
}

// TODO: Fix this mess
const HEADER: &str = r#"<!DOCTYPE html><html><head><meta name=viewport content="width=device-width, initial-scale=1.0"><meta name=keywords content="actix-drop"><meta name=author content=@artnoi><meta charset=UTF-8><link href=https://artnoi.com/style.css rel=stylesheet><title>actix-drop</title></head><body><h1><a href="/">actix-drop</a></h1>"#;
const FOOTER: &str = r#"<footer><p><a href="https://github.com/artnoi43/actix-drop">Contribute on Github</a></p></footer></body></html>"#;
const STYLE: &str = r#"html{overflow-y:scroll;-webkit-text-size-adjust:100%;-ms-text-size-adjust:100%;padding:1.5em}body{background:#000;margin:auto;max-width:80em;line-height:1.5em;font-size:18px;white-space:pre-wrap;word-wrap:break-word;color:#c0ca8e}pre>code{background:#161821;display:block;padding:10px 15px}footer p{font-family:Times;font-size:small;text-align:left}"#;

// Return HTML form for entering text to be saved
async fn landing_page() -> HttpResponse {
    HttpResponse::Ok().content_type("text/html").body(format!(
        r#"
                {}
                <form action="/drop" method="post">
                <textarea id="textbox" name="text" rows="4" cols="50"></textarea>
                <br>
                <button type="submit">Submit Clipboard</button>
                </form>
                {}
            "#,
        HEADER, FOOTER,
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
            .body("<p>error: blank clipboard sent</p>");
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
            .body(format!(
                r"
            {}
            <p>error: cannot save clipboard</p>
            {}
            ",
                HEADER, FOOTER,
            ));
    }

    form.text.truncate(10);
    let response = format!(
        r#"{0}
        <p>Clipboard {1} with hash <code>{2}</code> created</p>
        <p>The clipboard will be available at path <a href="/drop/{2}"><code>/drop/{2}</code></a></p>
        {3}
"#,
        HEADER, form.text, hash, FOOTER,
    );

    HttpResponse::Created()
        .content_type("text/html")
        .body(response)
}

// Retrive the clipboard based on its ID as per post_drop.
#[get("/drop/{id}")]
async fn get_drop(id: web::Path<String>) -> HttpResponse {
    match read_clipboard(id.clone().into()) {
        Err(err) => {
            eprintln!("read_file error: {}", err);

            return HttpResponse::NotFound()
                .content_type("text/html")
                .body(format!("error: no such clipboard: {}", id));
        }

        Ok(clipboard) => {
            let text = String::from_utf8(clipboard);
            if text.is_err() {
                return HttpResponse::InternalServerError()
                    .content_type("text/html")
                    .body("error: clipboard is non UTF-8");
            }

            let body = format!(
                r#"{}
                <p>Clipboard for <code>{}</code>:</p>
                <pre><code>{}</code></pre>
                {}

            "#,
                HEADER,
                id,
                text.unwrap(),
                FOOTER,
            );

            return HttpResponse::Ok().content_type("text/html").body(body);
        }
    }
}

async fn serve_css() -> HttpResponse {
    HttpResponse::Ok().content_type("text/css").body(STYLE)
}

fn write_clipboard<S>(name: S, content: &[u8]) -> std::io::Result<()>
where
    S: AsRef<Path>,
{
    let path = Path::new("drop/").join(name.as_ref());
    std::fs::write(path, content)
}

fn read_clipboard(id: web::Path<String>) -> std::io::Result<Vec<u8>> {
    let path = Path::new("drop/").join(id.as_ref());
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
    // Ensure that ./drop is a directory
    let check_result = check_dir("drop");
    match check_result {
        Ok(false) => create_dir("drop").expect("failed to create ./drop/"),

        Err(err) => match err.kind() {
            std::io::ErrorKind::NotFound => {
                create_dir("drop").expect("failed to create ./drop/");
            }

            _ => panic!("failed to get working directory information"),
        },

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

    println!("Serving on http://localhost:3000...");
    server
        .bind("127.0.0.1:3000")
        .expect("error binding server to address")
        .run()
        .await
        .expect("error running server");
}
