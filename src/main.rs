use actix_files as actix_fs;
use actix_web::{middleware::Logger, post, web, App, HttpResponse, HttpServer, Responder};
use ansi_to_html::convert;
use maud::{html, Markup, PreEscaped, DOCTYPE};
use serde::Deserialize;
use std::env;
use std::io::prelude::*;
use std::io::{Error, ErrorKind};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::{fs, fs::File};

mod middleware;

use crate::middleware::exclusive_controler::{ExclusiveLocker, ProcessStatus};

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

fn create_error_page(message: &str, stderr: &str) -> Markup {
    html! {
        (DOCTYPE)
        html lang="ja" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                style {(r##"
                body {margin: auto; width: 1000px;}
                pre {
                    background-color: #eee;
                    border-radius: 3px;
                    border: 1px solid black;
                    padding: 2px;
                }
                code {
                    font-family: courier, monospace;
                    padding: 0 3px;
                }
                "##)}
            }
            body {
                h3 {(message)}
                p {"error log"}
                pre { code {(PreEscaped(convert(&stderr,false, false).unwrap()))} }
            }
        }
    }
}

fn run_git_command(diary_repository_path: &str, args: &Vec<&str>) -> Result<(), Error> {
    let v = Command::new("git")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .args(args)
        .current_dir(&diary_repository_path)
        .spawn()?;
    let command_str = String::from("git ") + &args.join(" ");
    log::debug!("run command : {}", command_str);


    let output = v.wait_with_output()?;
    // エラー時の処理
    if !output.status.success() {
        let stderr = String::from_utf8(output.stderr).unwrap();
        log::error!("failed on run command: {}\n{}", command_str, stderr);
        let z = create_error_page(&(String::from("failed to run command: ") + &command_str), &stderr);
        return Err(Error::new(ErrorKind::InvalidData, z.into_string()));
    }
    log::debug!("{}", String::from_utf8(output.stdout).unwrap());
    Ok(())
}

fn commit_diary(req: web::Form<Diary>) -> Result<(), Error> {
    // レポジトリパスの取得
    let diary_repository_path = match env::var("DIARY_REPOSITORY_PATH") {
        Ok(v) => v,
        Err(e) => return Err(std::io::Error::new(std::io::ErrorKind::NotFound, e)),
    };
    // git pullの実行
    run_git_command(&diary_repository_path, &Vec::from(["pull", "origin", "main"]))?;
    // ファイルを作成
    let dates: Vec<&str> = req.date.split("-").collect();
    let path = format!("{}/diary/{}/{}/{}.md", diary_repository_path, dates[0], dates[1], dates[2]);
    {
        let parent = Path::new(&path).parent().unwrap();
        if !parent.exists() {
            fs::create_dir_all(parent.to_str().unwrap())?;
        }
        let mut file = File::create(&path)?;
        file.write_all(req.to_file().as_bytes())?;
    }
    // git add
    run_git_command(&diary_repository_path, &Vec::from(["add", "-A"]))?;
    // git commit
    run_git_command(
        &diary_repository_path,
        &Vec::from(["commit", "--all", "-m", &format!("{}/{}/{} (from web)", dates[0], dates[1], dates[2])]),
    )?;
    // git push
    run_git_command(&diary_repository_path, &Vec::from(["push", "origin", "HEAD"]))?;
    Ok(())
}

#[post("/diary")]
async fn post_diary(req: web::Form<Diary>) -> impl Responder {
    let response = html! {
        h1 {"updated!"}
        (PreEscaped(r#"<script>setTimeout(() => {window.location.pathname = ""}, 1000)</script>"#))
    };
    match commit_diary(req) {
        Ok(_) => HttpResponse::Ok().body(response.into_string()),
        Err(e) => HttpResponse::InternalServerError().body(format!("{}", e)),
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // RUST LOGSを出す
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();

    let args: Vec<String> = env::args().collect();

    let v = Arc::new(Mutex::new(ProcessStatus { running: false }));
    let arc = v.clone();
    
    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .wrap(ExclusiveLocker { working: Arc::clone(&arc) })
            .service(post_diary)
            .service(actix_fs::Files::new("/", &args[1]).index_file("index.html"))
    })
    .bind("0.0.0.0:8095")?
    .run()
    .await
}
