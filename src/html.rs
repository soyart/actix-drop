// TODO: Fix this mess
pub const HEADER: &str = r#"<!DOCTYPE html><html><head><meta name=viewport content="width=device-width, initial-scale=1.0"><meta name=keywords content="actix-drop"><meta name=author content=@artnoi><meta charset=UTF-8><link href=https://artnoi.com/style.css rel=stylesheet><title>actix-drop</title></head><body><h1><a href="/">actix-drop</a></h1>"#;
pub const FOOTER: &str = r#"<footer><p><a href="https://github.com/artnoi43/actix-drop">Contribute on Github</a></p></footer></body></html>"#;

pub fn wrap_html(s: &str) -> String {
    format!("{}{}{}", HEADER, s, FOOTER)
}
