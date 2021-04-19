use actix_files as actix_fs;
use actix_web::{post, web, App, HttpResponse, HttpServer, Responder};
use serde::Deserialize;
use std::env;
use std::io::prelude::*;
use std::path::Path;
use std::process::Command;
use std::{fs, fs::File};

#[derive(Deserialize)]
struct Diary {
    title: String,
    content: String,
    date: String,
}

impl Diary {
    fn to_file(&self) -> std::string::String {
        format!("---\ntitle: {}\n---\n\n{}", self.title, self.content)
    }
}

#[post("/diary")]
async fn post_diary(req: web::Form<Diary>) -> impl Responder {
    let z = || {
        let diary_repository_path = match env::var("DIARY_REPOSITORY_PATH") {
            Ok(v) => v,
            Err(e) => return Err(std::io::Error::new(std::io::ErrorKind::NotFound, e)),
        };
        // ファイルを作成
        let dates: Vec<&str> = req.date.split("-").collect();
        let path = format!(
            "{}/{}/{}/{}.md",
            diary_repository_path, dates[0], dates[1], dates[2]
        );
        let parent = Path::new(&path).parent().unwrap();
        if !parent.exists() {
            fs::create_dir_all(parent.to_str().unwrap())?;
        }
        let mut file = File::create(&path)?;
        file.write_all(req.to_file().as_bytes())?;
        // git pullの実行
        let mut v = Command::new("git")
            .args(&["pull", "origin", "HEAD"])
            .current_dir(&diary_repository_path)
            .spawn()?;
        v.wait()?;
        // git addの実行
        let mut v = Command::new("git")
            .args(&["add", "-A"])
            .current_dir(&diary_repository_path)
            .spawn()?;
        v.wait()?;
        // git commitの実行
        let message = format!("{}/{}/{} (from web)", dates[0], dates[1], dates[2]);
        let mut v = Command::new("git")
            .args(&["commit", "--all", "-m", &message])
            .current_dir(&diary_repository_path)
            .spawn()?;
        v.wait()?;
        // git pushの実行
        let mut v = Command::new("git")
            .args(&["push", "origin", "HEAD"])
            .current_dir(&diary_repository_path)
            .spawn()?;
        v.wait()?;
        Ok(true)
    };

    match z() {
        Ok(_) => HttpResponse::Ok().body("updated!"),
        Err(e) => HttpResponse::InternalServerError().body(format!("{}", e)),
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .service(post_diary)
            .service(actix_fs::Files::new("/", "./static").index_file("index.html"))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
