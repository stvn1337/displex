use axum::{
    extract::{
        Query,
        State,
    },
    response::{
        IntoResponse,
        Redirect,
    },
    routing::get,
    Router,
};
use axum_sessions::extractors::WritableSession;
use serde::Deserialize;

use crate::{
    errors::DisplexError,
    server::axum::DisplexState,
    session::{
        DISCORD_CODE,
        DISCORD_STATE,
    },
};

async fn linked_role(
    mut session: WritableSession,
    State(state): State<DisplexState>,
) -> Result<impl IntoResponse, DisplexError> {
    let (url, state) = state.discord_client.authorize_url();
    session.insert(DISCORD_STATE, state.secret())?;

    Ok(Redirect::to(url.as_str()))
}

#[derive(Deserialize)]
struct CallbackQueryParams {
    pub code: String,
    pub state: String,
}

async fn callback(
    mut session: WritableSession,
    State(state): State<DisplexState>,
    query_string: Query<CallbackQueryParams>,
) -> Result<impl IntoResponse, DisplexError> {
    let session_state = session
        .get::<String>(DISCORD_STATE)
        .ok_or_else(|| anyhow::anyhow!("no state found in session"))?;
    verify_state(&session_state, &query_string.state)?;

    session.insert(DISCORD_CODE, &query_string.code)?;

    let pin = state.plex_client.get_pin().await?;
    let url = state
        .plex_client
        .generate_auth_url(pin.id, &pin.code)
        .await?;

    Ok(Redirect::to(url.as_str()))
}

#[tracing::instrument]
fn verify_state(session_state: &str, query_string_state: &str) -> Result<(), anyhow::Error> {
    println!("got here");
    if session_state != query_string_state {
        tracing::info!("session state does not match query parameters");
        anyhow::bail!("invalid state")
    }
    Ok(())
}

pub fn routes() -> Router<DisplexState> {
    Router::new()
        .route("/discord/linked-role", get(linked_role))
        .route("/discord/callback", get(callback))
}
