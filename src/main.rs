use std::sync::Mutex;
use std::io::{self};
use actix_web::{http::header, web, HttpServer, HttpResponse, App, Responder};
use sse_actix_web::{Broadcaster, broadcast};
use sled;
use actix_files::NamedFile;
use actix_cors::Cors;

pub struct MyData {
    db: sled::Db
}

async fn new_client(data: web::Data<MyData>, broadcaster: web::Data<Mutex<Broadcaster>>) -> impl Responder {

    let counter_buffer = data.db.get(b"counter").unwrap().unwrap();
    
    let counter = std::str::from_utf8(&counter_buffer).unwrap();

    let rx = broadcaster.lock().unwrap().new_client(&"counter", counter);

    HttpResponse::Ok()
        .header("content-type", "text/event-stream")
        .no_chunking()
        .streaming(rx)
}

async fn download(data: web::Data<MyData>, broad: web::Data<std::sync::Mutex<Broadcaster>>) -> io::Result<NamedFile> {
    
    let counter_buffer = data.db.get(b"counter").unwrap().unwrap();
    
    let counter = std::str::from_utf8(&counter_buffer).unwrap();
    
    let counter_int = counter.clone().parse::<i32>().unwrap();
    
    let new_counter_int = counter_int + 1;
    
    let new_counter_string = new_counter_int.clone().to_string();
    
    let new_counter = new_counter_string.as_bytes();
    
    let old_counter = counter.clone().as_bytes();

    let _ = data.db.compare_and_swap(b"counter", Some(old_counter.clone()), Some(new_counter.clone()));

    let _ = web::block(move || data.db.flush()).await;

    broadcast("counter".to_owned(), new_counter_string.to_owned(), broad).await;

    let f = web::block(|| std::fs::File::create("test.pdf")).await.unwrap();
    
    NamedFile::from_file(f, "test.pdf")
}

async fn index() -> impl Responder {

    let content = r#"<html lang="en">
    <head>
        <meta charset="UTF-8">
        <meta name="viewport" content="width=device-width, initial-scale=1.0">
        <meta http-equiv="X-UA-Compatible" content="ie=edge">
        <title>Server-sent events</title>
        <style>
            p {
                margin-top: 0.5em;
                margin-bottom: 0.5em;
            }
        </style>
    </head>
    <body>
        <div>Files Downloaded: <span id="root"></span></div>
        <div><a href="/download" target="_blank">Download</a></div>
        <script>
            let root = document.getElementById("root");
            let events = new EventSource("/events");
            let data = document.createElement("p");
            root.appendChild(data);
            events.addEventListener("counter", (event) => {
                data.innerText = event.data;
            });
        </script>
    </body>
    </html>"#;

    HttpResponse::Ok()
        .header("content-type", "text/html")
        .body(content)
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    
    let tree = sled::open("./tmp/data").unwrap();
    let tree_clone = tree.clone();
    
    let _ = tree.compare_and_swap(b"counter", None as Option<&[u8]>, Some(b"0"));
    
    let _ = web::block(move || tree.flush()).await;

    let data = Broadcaster::create();

    HttpServer::new(move || {
        App::new()
            .wrap(
                Cors::new()
                    .send_wildcard()
                    .allowed_methods(vec!["GET", "POST"])
                    .allowed_headers(vec![header::AUTHORIZATION, header::ACCEPT, header::CONTENT_TYPE])
                    .max_age(3600)
                    .finish(),
            )
            .app_data(data.clone())
            .data(MyData{ db: tree_clone.clone()})
            .route("/", web::get().to(index))
            .route("/events", web::get().to(new_client))
            .route("/download", web::get().to(download))
    })
    .bind("0.0.0.0:3000")?
    .run()
    .await
}
