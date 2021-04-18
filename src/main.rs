use actix_files as fs;
use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use serde::Deserialize;

#[derive(Deserialize)]
struct Diary {
    title: String,
    content: String,
    date: String,
}

#[post("/diary")]
async fn post_diary(req: web::Form<Diary>) -> impl Responder {
    HttpResponse::Ok().body(&req.date)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .service(post_diary)
            .service(fs::Files::new("/", "./static").index_file("index.html"))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
