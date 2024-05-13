mod http_resp;
mod http_server;

#[cfg(unix)] // Our code currently uses UNIX file paths
#[actix_web::main]
async fn main() {
    use std::time::Duration;

    use actix_web::{middleware, web, App, HttpServer};
    use colored::Colorize;

    use soyjot::config::AppConfig;
    use soyjot::store::{self, tracker::Tracker};

    let conf = AppConfig::init();
    println!(
        "\n{}\n{}\n",
        "Starting actix-drop: current configuration".yellow(),
        serde_json::to_string(&conf).unwrap()
    );

    // Ensure that ./${DIR} is a directory
    store::persist::assert_dir(conf.dir);

    let http_addr = format!(
        "{}:{}",
        conf.http_addr.expect(&"http_addr is None".red()),
        conf.http_port.expect(&"http_port is None".red()),
    );

    println!(
        "{} {}",
        "Starting actix-web on".yellow(),
        format!("http://{}", http_addr).cyan()
    );

    HttpServer::new(move || {
        App::new()
            .wrap(middleware::NormalizePath::new(
                middleware::TrailingSlash::Trim,
            ))
            .app_data(web::Data::new(Duration::from_secs(
                conf.timeout.expect("timeout is None"),
            )))
            .app_data(web::Data::new(String::from(http_server::CSS)))
            .app_data(web::Data::new(Tracker::new()))
            .service(web::resource("/style.css").route(web::get().to(http_server::serve_css)))
            .service(http_server::routes::<http_resp::ResponseHtml>("/app"))
            .service(http_server::routes::<http_resp::ResponseJson>("/api"))
            .service(http_server::routes::<http_resp::ResponseText>("/txt"))
    })
    .bind(http_addr)
    .expect(&"error binding server to address".red())
    .run()
    .await
    .expect(&"error running server".red());
}
