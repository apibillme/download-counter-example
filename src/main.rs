use std::io::{self};
use actix_web::{web, HttpServer, HttpResponse, App, Responder};
use sse_actix_web::{new_client, Broadcaster, broadcast};
use sled::Db;
use actix_files::NamedFile;

pub struct MyData {
    db: sled::Db
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

    broadcast(new_counter_string.to_owned(), broad).await;

    let f = web::block(|| std::fs::File::create("test.pdf")).await.unwrap();
    
    NamedFile::from_file(f, "test.pdf")
}

async fn index(data: web::Data<MyData>) -> impl Responder {

    let counter_buffer = data.db.get(b"counter").unwrap().unwrap();
    
    let counter = std::str::from_utf8(&counter_buffer).unwrap();

    let a = r#"<html lang="en">
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
        <div id="root"></div>
        <script>
            let root = document.getElementById("root");
            let events = new EventSource("/events");
            let data = document.createElement("p");
            root.appendChild(data);
            data.innerText = "#;
        let b = format!("\"{}\";\n", counter);
        let c = r#"
            events.onmessage = (event) => {
                if (event.data != "connected") {
                    data.innerText = event.data;
                }
            }
        </script>
    </body>
    </html>"#;

    let content = format!("{}{}{}", a, b, c);

    HttpResponse::Ok()
        .header("content-type", "text/html")
        .body(content)
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    let tree = Db::open("./tmp/data").unwrap();
    let data = Broadcaster::create("connected".to_owned());

    HttpServer::new(move || {
        App::new()
            .register_data(data.clone())
            .data(MyData{ db: tree.clone()})
            .route("/", web::get().to(index))
            .route("/events", web::get().to(new_client))
            .route("/download", web::get().to(download))
    })
    .bind("0.0.0.0:3000")?
    .start()
    .await
}
