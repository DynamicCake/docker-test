use std::{thread, time::Duration};

use const_format::concatcp;
use poem::{listener::TcpListener, Route};
use poem_openapi::{param::Query, payload::PlainText, ApiResponse, OpenApi, OpenApiService};
use redis::{aio::MultiplexedConnection, AsyncCommands};
use tokio::main;
use tracing::{error, info};

const PORT: u16 = 80;

#[main]
async fn main() {
    color_eyre::install().unwrap();
    // Logging setup
    let tracing_sub = tracing_subscriber::fmt()
        .compact()
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_target(false)
        .finish();

    tracing::subscriber::set_global_default(tracing_sub).unwrap();

    info!("trying to connect");
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let connection = loop {
        match client.get_multiplexed_tokio_connection().await {
            Ok(it) => break it,
            Err(err) => { 
                thread::sleep(Duration::new(1, 0));
                error!("{}", err);
            },
        };
    };

    info!("connected!");
    let main_service = OpenApiService::new(Api::new(connection), "Main", "1.0.0")
        .server(concatcp!("http://localhost:", PORT, "/api"));

    let app = Route::new().nest("/api", main_service);

    let listener = TcpListener::bind(("0.0.0.0", PORT));
    poem::Server::new(listener).run(app).await.unwrap();
    info!("done")
}

struct Api {
    con: MultiplexedConnection,
}

impl Api {
    fn new(con: MultiplexedConnection) -> Self {
        Self { con }
    }
}

#[OpenApi]
impl Api {
    #[oai(path = "/ping", method = "get")]
    async fn ping(&self) -> PlainText<&'static str> {
        PlainText("Pong")
    }
    #[oai(path = "/get", method = "get")]
    async fn get(&self, key: Query<String>) -> PlainText<String> {
        let mut con = self.con.clone();
        let res: Option<String> = con.get(key.0).await.unwrap();
        PlainText(res.unwrap_or("None".to_string()))
    }

    #[oai(path = "/set", method = "put")]
    async fn set(&self, key: Query<String>, value: Query<String>) -> SetResponse {
        let mut con = self.con.clone();
        let set: Result<(), _> = con.set(key.0, value.0).await;
        match set {
            Ok(_) => SetResponse::Ok,
            Err(_) => SetResponse::Err,
        }
    }
}

#[derive(ApiResponse)]
enum SetResponse {
    #[oai(status = "200")]
    Ok,
    #[oai(status = "400")]
    Err,
}
