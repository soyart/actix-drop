const HEADER: &str = r#"<!DOCTYPE html><html><head><meta name=viewport content="width=device-width, initial-scale=1.0"><meta name=keywords content="actix-drop"><meta name=author content=@artnoi><meta charset=UTF-8><link href=https://artnoi.com/style.css rel=stylesheet><title>actix-drop</title></head><body><h1><a href="/">actix-drop</a></h1>"#;
const FOOTER: &str = r#"<footer><p><a href="https://github.com/artnoi43/actix-drop">Contribute on Github</a></p></footer></body></html>"#;

#[macro_export]
macro_rules! tag_html {
    ( $key: expr, $val: expr ) => {
        format!("<{0}>{1}</{0}>", $key, $val)
    };
}

#[macro_export]
macro_rules! para {
    ( $v: expr ) => {
        tag_html!("p", $v)
    };
}

#[macro_export]
macro_rules! code {
    ( $v: expr ) => {
        tag_html!("code", $v)
    };
}

pub fn wrap_html(s: &str) -> String {
    format!("{}{}{}", HEADER, s, FOOTER)
}

#[cfg(test)]
mod tests_html {
    #[test]
    fn test_html() {
        assert_eq!(para!("foo"), "<p>foo</p>".to_string());
        assert_eq!(para!(code!("foo")), "<p><code>foo</code></p>".to_string());
    }
}
