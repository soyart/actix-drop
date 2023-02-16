use serde::{Deserialize, Serialize};

const DIR: &'static str = "./drop";
const HTTP_ADDR: &'static str = "127.0.0.1";
const HTTP_PORT: u16 = 8080;
const TIMEOUT: u64 = 15;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct AppConfig {
    pub dir: Option<String>,
    pub http_addr: Option<String>,
    pub http_port: Option<u16>,
    pub timeout: Option<u64>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            dir: Some(DIR.to_string()),
            http_addr: Some(HTTP_ADDR.to_string()),
            http_port: Some(HTTP_PORT),
            timeout: Some(TIMEOUT),
        }
    }
}

impl AppConfig {
    pub fn init() -> Self {
        match init_config() {
            Ok(conf) => conf,
            Err(err) => {
                eprintln!("error reading AppConfig, using default..: {err:?}");
                Self::default()
            }
        }
    }
}

fn init_config() -> Result<AppConfig, config::ConfigError> {
    config::Config::builder()
        .set_default("dir", DIR)?
        .set_default("http_addr", HTTP_ADDR)?
        .set_default("http_port", HTTP_PORT)?
        .set_default("timeout", TIMEOUT.to_string())?
        .add_source(config::File::with_name("/etc/actix-drop/config").required(false))
        .add_source(config::File::with_name("$HOME/.config/actix-drop/config").required(false))
        .add_source(config::File::with_name("$HOME/.actix-drop/config").required(false))
        .add_source(config::Environment::with_prefix("DROP"))
        .build()?
        .try_deserialize::<AppConfig>()
}

#[cfg(test)]
mod tests {
    use super::AppConfig;

    const DIR: &str = "./foo";
    const ADDR: &str = "192.168.1.1";
    const PORT: u16 = 6969;
    const TIMEOUT: u64 = 69;

    macro_rules! assert_eq_test_default {
        ( $conf: expr ) => {
            assert_eq!(
                $conf,
                AppConfig {
                    dir: Some(DIR.to_string()),
                    http_addr: Some(ADDR.to_string()),
                    http_port: Some(PORT),
                    timeout: Some(TIMEOUT),
                }
            )
        };
    }

    #[test]
    fn test_config_deserialize() {
        use serde_json::json;

        let j = json!({
            "timeout": TIMEOUT,
            "http_addr": ADDR,
            "dir": DIR,
        })
        .to_string();

        let conf = serde_json::from_str::<AppConfig>(&j).expect("failed to deserialize json");
        assert_eq!(conf.dir, Some(DIR.to_string()));
        assert_eq!(conf.timeout, Some(TIMEOUT));
        assert_eq!(conf.http_port, None);
        assert_eq!(conf.http_addr, Some(ADDR.to_string()));
    }

    #[test]
    fn test_env_config() {
        use std::env;

        env::set_var("DROP_DIR", DIR);
        env::set_var("DROP_HTTP_ADDR", ADDR);
        env::set_var("DROP_HTTP_PORT", PORT.to_string());
        env::set_var("DROP_TIMEOUT", TIMEOUT.to_string());

        let conf = config::Config::builder()
            .add_source(config::Environment::with_prefix("drop"))
            .build()
            .expect("failed to build")
            .try_deserialize::<AppConfig>()
            .unwrap();

        assert_eq_test_default!(conf);
    }

    #[test]
    fn test_init_config() {
        use super::init_config;
        use std::env;

        env::set_var("DROP_DIR", DIR);
        env::set_var("DROP_HTTP_ADDR", ADDR);
        env::set_var("DROP_HTTP_PORT", PORT.to_string());
        env::set_var("DROP_TIMEOUT", TIMEOUT.to_string());

        let conf = init_config().expect("init_config failed");
        println!("test_init_config: {conf:?}");

        assert_eq_test_default!(conf);
    }
}
