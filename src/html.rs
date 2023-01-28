// TODO: Fix this mess
pub const HEADER: &str = r#"<!DOCTYPE html><html><head><meta name=viewport content="width=device-width, initial-scale=1.0"><meta name=keywords content="actix-drop"><meta name=author content=@artnoi><meta charset=UTF-8><link href=https://artnoi.com/style.css rel=stylesheet><title>actix-drop</title></head><body><h1><a href="/">actix-drop</a></h1>"#;
pub const FOOTER: &str = r#"<footer><p><a href="https://github.com/artnoi43/actix-drop">Contribute on Github</a></p></footer></body></html>"#;
pub const STYLE: &str = r#"html{overflow-y:scroll;-webkit-text-size-adjust:100%;-ms-text-size-adjust:100%;padding:1.5em}body{background:#000;margin:auto;max-width:80em;line-height:1.5em;font-size:18px;white-space:pre-wrap;word-wrap:break-word;color:#c0ca8e}pre>code{background:#161821;display:block;padding:10px 15px}footer p{font-family:Times;font-size:small;text-align:left}"#;

pub fn wrap_html(s: &str) -> String {
    format!("{}{}{}", HEADER, s, FOOTER)
}
