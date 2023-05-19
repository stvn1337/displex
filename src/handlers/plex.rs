use std::time::Duration;

use actix_session::Session;
use actix_web::{
    error::{ErrorInternalServerError, ErrorUnauthorized},
    web::{self, Redirect},
    Responder, Result,
};
use oauth2::TokenResponse;
use serde::Deserialize;

use crate::{
    config::Config,
    db::{
        self,
        discord::{NewDiscordToken, NewDiscordUser},
        plex::{NewPlexToken, NewPlexUser},
        DbPool,
    },
    discord::{
        client::DiscordClient,
        models::{ApplicationMetadata, ApplicationMetadataUpdate},
    },
    plex::client::PlexClient,
    session,
    tautulli::{client::TautulliClient, models::QueryDays},
};

#[derive(Debug, Deserialize)]
pub struct PlexRedirectQueryParams {
    pub id: u64,
    pub code: String,
}

pub async fn callback(
    config: web::Data<Config>,
    discord_client: web::Data<DiscordClient>,
    plex_client: web::Data<PlexClient>,
    pool: web::Data<DbPool>,
    qs: web::Query<PlexRedirectQueryParams>,
    session: Session,
    tautulli_client: web::Data<TautulliClient>,
) -> Result<impl Responder> {
    let resp = plex_client
        .pin_claim(qs.id, &qs.code)
        .await
        .map_err(|err| {
            log::error!("{}", err);
            ErrorInternalServerError("something bad happened")
        })?;

    let discord_token = session
        .get::<String>(session::DISCORD_CODE)?
        .expect("invalid discord token");

    match plex_client
        .get_devices(&resp.auth_token)
        .await
        .map_err(|err| {
            log::error!("{}", err);
            ErrorInternalServerError("something bad happened")
        })?
        .iter()
        .find(|&d| d.client_identifier == config.plex_server_id)
    {
        Some(_) => {
            let token = discord_client.token(&discord_token).await.map_err(|err| {
                log::error!("discord_client.token: {}", err);
                ErrorInternalServerError("something bad happened")
            })?;

            let d_access_token = String::from(token.access_token().secret());
            let t = token.clone();

            let discord_user = discord_client.user(&d_access_token).await.map_err(|err| {
                log::error!("discord_client.user: {}", err);
                ErrorInternalServerError("something bad happened")
            })?;

            let plex_user = plex_client.user(&resp.auth_token).await.map_err(|err| {
                log::error!("plex_client.user {}", err);
                ErrorInternalServerError("something bad happened")
            })?;

            web::block(move || {
                // note that obtaining a connection from the pool is also potentially blocking
                let mut conn = pool.get()?;

                conn.build_transaction().run::<_, anyhow::Error, _>(|conn| {
                    let discord_user = db::discord::insert_user(
                        conn,
                        NewDiscordUser {
                            id: discord_user.id,
                            username: discord_user.username,
                        },
                    )?;
                    log::debug!("inserted discord user: {:?}", discord_user);

                    let discord_token = db::discord::insert_token(
                        conn,
                        NewDiscordToken {
                            access_token: token.access_token().secret().into(),
                            refresh_token: t
                                .refresh_token()
                                .expect("expecting refresh token")
                                .secret()
                                .into(),
                            scopes: t.scopes().map_or("".into(), |d| {
                                d.iter().map(|i| i.to_string() + ",").collect()
                            }),
                            expires_at: chrono::Utc::now()
                                + chrono::Duration::seconds(
                                    token
                                        .expires_in()
                                        .unwrap_or(Duration::from_secs(1800))
                                        .as_secs() as i64,
                                ),
                            discord_user_id: String::from(&discord_user.id),
                        },
                    )?;
                    log::debug!("inserted discord token: {:?}", discord_token);

                    let plex_user = db::plex::insert_user(
                        conn,
                        NewPlexUser {
                            id: plex_user.id,
                            username: plex_user.username,
                            discord_user_id: String::from(&discord_user.id),
                        },
                    )?;
                    log::debug!("inserted plex user: {:?}", plex_user);

                    let plex_token = db::plex::insert_token(
                        conn,
                        NewPlexToken {
                            access_token: resp.auth_token,
                            plex_user_id: plex_user.id,
                        },
                    )?;
                    log::debug!("inserted plex token: {:?}", plex_token);

                    Ok(())
                })
            })
            .await?
            // map diesel query errors to a 500 error response
            .map_err(|err| {
                log::error!("db save: {}", err);
                ErrorInternalServerError("something bad happened")
            })?;

            let watch_stats = tautulli_client
                .get_user_watch_time_stats(plex_user.id, Some(true), Some(QueryDays::Total))
                .await
                .map_err(|err| {
                    log::error!("tautulli_client.get_user_watch_time_stats: {}", err);
                    ErrorInternalServerError("something bad happened")
                })?;

            let mut data = ApplicationMetadata {
                ..Default::default()
            };
            if let Some(latest) = watch_stats.get(0) {
                data.total_watches = latest.total_plays;
                data.hours_watched = latest.total_time / 3600;
            }

            discord_client
                .link_application(
                    &d_access_token,
                    ApplicationMetadataUpdate {
                        platform_name: String::from(&config.application_name),
                        metadata: data,
                    },
                )
                .await
                .map_err(|err| {
                    log::error!("discord_client.link_application: {}", err);
                    ErrorInternalServerError("something bad happened")
                })?;
            Ok(Redirect::to(discord_client.generate_auth_success_url()))
        }
        None => Err(ErrorUnauthorized("unauthorized user")),
    }
}