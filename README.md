# DisPlex

Discord & Plex & Tautulli (& more maybe one day) application.

## Features

### Discord Linked Role

You can use this bot to create a [Discord Linked Roles](https://support.discord.com/hc/en-us/articles/8063233404823-Connections-Linked-Roles-Community-Members) which verifies the user has access to your Plex instance.
It requires you to deploy the app behind proper HTTPS certificates, [cloudflared](https://github.com/cloudflare/cloudflared) can be used during developement to easily issue valid SSL certs in testing.

## Getting Started

1. [Create a Discord Application](https://discord.com/developers/applications)
1. Generate the configuration.
```bash
> cat .env
# General App Settings
DISPLEX_HOSTNAME="displex.example.com"
DISPLEX_APPLICATION_NAME="MyPlex"
# Can generate with `openssl rand -hex 32`
DISPLEX_SESSION_SECRET_KEY="secret"
DISPLEX_ACCEPT_INVALID_CERTS=false  # optional

# visit https://<plex>:32400/identity the value of 'machineIdentifier'
DISPLEX_PLEX_SERVER_ID="plex-server-id"

# Discord Application Settings
DISPLEX_DISCORD_CLIENT_ID="clientid"
DISPLEX_DISCORD_CLIENT_SECRET="clientsecret"
DISPLEX_DISCORD_BOT_TOKEN="bottoken"
DISPLEX_DISCORD_SERVER_ID="serverid"
DISPLEX_DISCORD_CHANNEL_ID="channelid"

DISPLEX_TAUTULLI_API_KEY="apikey"
DISPLEX_TAUTULLI_URL="https://tautulli.example.com"
```
3. Populate settings in Discord Application
   1. `Linked Roles Verification URL` = `https://displex.example.com/discord/linked-role`
   1. `OAuth2 Redirects` = `https://displex.example.com/discord/callback`

4. Associate the App to a Roles Links in the Discord server.

### Environment Variables

| Name                          | Description                                                                                             | Required | Default   |
|-------------------------------|---------------------------------------------------------------------------------------------------------|----------|-----------|
| DISPLEX_HOSTNAME              | Hostname of application. Used to generate the redirect URLs for OAuth2.                                 | yes      |           |
| DISPLEX_APPLICATION_NAME      | Name of application. Will be displayed on Plex Sign-in mostly.                                          | yes      |           |
| DISPLEX_SERVICE_HOST          | Host to bind HTTP server.                                                                               | no       | 127.0.0.1 |
| DISPLEX_SERVICE_PORT          | Port to bind HTTP server                                                                                | no       | 8080      |
| DISPLEX_SESSION_SECRET_KEY    | Session secret value for encryption. Mostly used in OAuth2 flow to store state between requests         | yes      |           |
| DISPLEX_DATABASE_URL          | PostgresQL database url. For example postgres://displex:password@localhost/displex                      | yes      |           |
| DISPLEX_ACCEPT_INVALID_CERTS  | Control whether reqwest will validate SSL certs. Useful for MITM proxy development.                     | no       | false     |
| DISPLEX_PLEX_SERVER_ID        | Plex Server ID. When a user attempts to link the role, it will check if they have access to the server. | yes      |           |
| DISPLEX_DISCORD_CLIENT_ID     | Discord Application Client ID.                                                                          | yes      |           |
| DISPLEX_DISCORD_CLIENT_SECRET | Discord Application Client Secret.                                                                      | yes      |           |
| DISPLEX_DISCORD_BOT_TOKEN     | Discord Application Bot Token. Only used at the moment to register the application metadata.            | yes      |           |
| DISPLEX_DISCORD_SERVER_ID     | Discord Server ID, used for the redirect back to Discord after authorization flow.                      | yes      |           |
| DISPLEX_DISCORD_CHANNEL_ID    | Discord Channel ID, used for the redirect back to Discord after authorization flow.                     | yes      |           |
| DISPLEX_TAUTULLI_API_KEY      | Tautulli API key.                                                                                       | yes      |           |
| DISPLEX_TAUTULLI_URL          | URL to Tautulli server. For example https://localhost:8181                                              | yes      |           |

## Development

Checkout [docker-compose.yaml](./docker-compose.yaml) for a sample of how you can easily setup a dev environment. [Cloudflare Tunnel](https://developers.cloudflare.com/cloudflare-one/connections/connect-apps/install-and-setup/tunnel-guide/) (or similiar) can be used to easily put your development instance behind HTTPS with valid certificates which are needed for the OAuth2 flow with Discord.

# For Development if on MacOS you have to run cloudflared via CLI since docker host networking doesn't work.
1. Setup Cloudflare tunnel
1. Setup Discord Application
1. Setup Tautulli
1. Setup diesel_cli
# Can just launch mitm/postgres as its easier to `cargo run`
1. `docker-compose up mitm postgres -d`
1. `mv .example.env .env` and fix values.
1. `diesel migrations run`
1. `cargo run`

By default `HTTPS_PROXY` will make `reqwest` route all traffic through the MITM proxy running at `http://localhost:8081`, which is useful for development.
