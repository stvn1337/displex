use std::{time::Duration, env};

use actix_session::{storage::CookieSessionStore, Session, SessionMiddleware};
use actix_web::{
    cookie::Key,
    error::{ErrorBadRequest, ErrorUnauthorized},
    get,
    middleware::Logger,
    web::{self, Redirect},
    App, HttpResponse, HttpServer, Responder, Result,
};

use config::Config;
use dotenv::dotenv;
use reqwest::header::HeaderValue;
use serde::Deserialize;

use crate::{discord::client::DiscordClient, plex::client::PlexClient};

mod config;
mod discord;
mod plex;
mod session;

// 1. Initial route that will ask user to authorize bot for their discord account
#[get("/discord/linked-role")]
async fn discord_linked_role(
    discord_client: web::Data<DiscordClient>,
    session: Session,
) -> Result<impl Responder> {
    let (url, state) = discord_client.authorize_url();
    session.insert(session::DISCORD_STATE, state.secret())?;

    Ok(Redirect::to(url.to_string()))
}

#[derive(Debug, Deserialize)]
pub struct DiscordRedirectQueryParams {
    pub code: String,
    pub state: String,
}

// 2. URL Discord will redirect user to after granting bot access
#[get("/discord/callback")]
async fn discord_callback(
    plex_client: web::Data<PlexClient>,
    qs: web::Query<DiscordRedirectQueryParams>,
    session: Session,
) -> Result<impl Responder> {
    let session_token = session.get::<String>(session::DISCORD_STATE)?.expect("invalid state");
    if session_token != qs.state {
        log::info!("session state does not match query parameters");
        Err(ErrorBadRequest("invalid state"))
    } else {
        session.insert(session::DISCORD_CODE, &qs.code)?;

        let pin = plex_client.get_pin().await;
        let url = plex_client.generate_auth_url(pin.id, &pin.code).await;

        Ok(Redirect::to(String::from(url)))
    }
}

#[derive(Debug, Deserialize)]
pub struct PlexRedirectQueryParams {
    pub id: u64,
    pub code: String,
}

// 3. Callback plex will redirect to after user grants access
#[get("/plex/callback")]
async fn plex_callback(
    config: web::Data<Config>,
    discord_client: web::Data<DiscordClient>,
    plex_client: web::Data<PlexClient>,
    qs: web::Query<PlexRedirectQueryParams>,
    session: Session,
) -> Result<impl Responder> {
    let resp = plex_client.pin_claim(qs.id, &qs.code).await;

    let discord_token = session
        .get::<String>(session::DISCORD_CODE)?
        .expect("invalid discord token");

    match plex_client
        .get_devices(&resp.auth_token)
        .await
        .iter()
        .find(|&d| d.client_identifier == config.plex_server_id)
    {
        Some(_) => {
            let token = discord_client.token(&discord_token).await;
            discord_client.link_application(&token).await;
            Ok(HttpResponse::Ok()
                .body("Successfully linked! You can go back to Discord now and close this tab."))
        }
        None => Err(ErrorUnauthorized("unauthorized user")),
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();

    let config = config::Config::init();
    let cfg = config.clone();
    let port = config.port;

    let mut default_headers = reqwest::header::HeaderMap::new();
    default_headers.append("Accept", HeaderValue::from_static("application/json"));

    let reqwest_client = reqwest::ClientBuilder::new()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .pool_idle_timeout(Duration::from_secs(90))
        .default_headers(default_headers)
        .build()
        .unwrap();

    env::set_var("RUST_LOG", "actix_web=debug,actix_server=info");
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("debug"));
    HttpServer::new(move || {
        App::new()
            .service(discord_callback)
            .service(discord_linked_role)
            .service(plex_callback)
            .app_data(web::Data::new(plex::client::PlexClient::new_with_client(
                reqwest_client.clone(),
                &config.plex_client_id,
                &format!("https://{}/plex/callback", &config.hostname),
            )))
            .app_data(web::Data::new(DiscordClient::new(
                reqwest_client.clone(),
                &config.discord_client_id,
                &config.discord_client_secret,
                &format!("https://{}/discord/callback", &config.hostname),
            )))
            .app_data(web::Data::new(config.clone()))
            .wrap(Logger::default())
            .wrap(
                // create cookie based session middleware
                SessionMiddleware::builder(
                    CookieSessionStore::default(),
                    Key::from(&config.session_secret_key.as_bytes()),
                )
                .cookie_secure(true)
                .build(),
            )
    })
    .bind((cfg.host, port))?
    .run()
    .await
}
