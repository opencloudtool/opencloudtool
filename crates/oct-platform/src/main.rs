use askama::Template;
use std::collections::HashMap;
use std::{env, fs};
use tower_http::trace::{self, TraceLayer};

use axum::{
    Router,
    extract::Query,
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
    routing::get,
};
use serde::{Deserialize, Serialize};

/// Runs the application server.
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    let github_config =
        GithubConfig::new().expect("Failed to initialize `GithubConfig`, check env variables");

    let app = Router::new()
        .route("/", get(index))
        .route("/repos", get(list_repos))
        .route("/login/github", get(github_login))
        .route("/login/github/redirect", get(github_login_redirect))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(trace::DefaultMakeSpan::new().level(tracing::Level::INFO))
                .on_response(trace::DefaultOnResponse::new().level(tracing::Level::INFO)),
        )
        .with_state(github_config);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .expect("Failed to bind listener to 0.0.0.0:8080");

    axum::serve(listener, app)
        .await
        .expect("Failed to start server");
}

/// Env state passed to the endpoints.
#[derive(Clone)]
struct GithubConfig {
    client_id: String,
    client_secret: String,
}

impl GithubConfig {
    const CLIENT_ID_ENV_NAME: &str = "GITHUB_CLIENT_ID";
    const CLIENT_SECRET_ENV_NAME: &str = "GITHUB_CLIENT_SECRET";

    /// Tries to create a new ``GithubConfig``
    fn new() -> Result<Self, env::VarError> {
        let client_id = env::var(Self::CLIENT_ID_ENV_NAME)?;
        let client_secret = env::var(Self::CLIENT_SECRET_ENV_NAME)?;

        Ok(GithubConfig {
            client_id,
            client_secret,
        })
    }
}

/// Index page template
#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate;

/// Github repo info page template
#[derive(Template)]
#[template(path = "repo.html")]
struct RepoTemplate<'a> {
    username: &'a str,
}

/// Renders the index page.
async fn index() -> impl IntoResponse {
    let index_template = IndexTemplate;

    match index_template.render() {
        Ok(response) => (StatusCode::OK, Html(response)),
        Err(_) => (
            StatusCode::BAD_REQUEST,
            Html(String::from("Failed to render `IndexTemplate`")),
        ),
    }
}

/// Renders the repo list page.
async fn list_repos() -> impl IntoResponse {
    let Ok(user) = User::load() else {
        return (
            StatusCode::BAD_REQUEST,
            Html(String::from("Failed to load from `user.json`")),
        );
    };

    let repo_template = RepoTemplate {
        username: &user.login,
    };

    match repo_template.render() {
        Ok(response) => (StatusCode::OK, Html(response)),
        Err(_) => (
            StatusCode::BAD_REQUEST,
            Html(String::from("Failed to render `RepoTemplate`")),
        ),
    }
}

/// Handles the login to Github.
async fn github_login(State(github_config): State<GithubConfig>) -> impl IntoResponse {
    (
        StatusCode::OK,
        [(
            "HX-Redirect",
            format!(
                "https://github.com/login/oauth/authorize?client_id={client_id}&login",
                client_id = github_config.client_id
            ),
        )],
        "OK",
    )
}

/// Github access token response.
#[derive(Deserialize)]
struct AccessTokenResponse {
    access_token: String,
}

/// Github user data response.
#[derive(Deserialize)]
struct UserDataResponse {
    login: String,
}

/// Holds the user information.
#[derive(Serialize, Deserialize)]
struct User {
    login: String,
    access_token: String,
}

impl User {
    /// Loads the user data from `user.json` file.
    fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let existing_data = fs::read_to_string("user.json")?;

        Ok(serde_json::from_str::<Self>(&existing_data)?)
    }

    /// Saves the user data to `user.json` file.
    fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        fs::write("user.json", serde_json::to_string_pretty(self)?)?;

        Ok(())
    }
}

/// Handles the redirect from Github after login.
async fn github_login_redirect(
    State(github_config): State<GithubConfig>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let Some(code) = params.get("code") else {
        return (
            StatusCode::BAD_REQUEST,
            Html(String::from("`code` is not provided")),
        )
            .into_response();
    };

    let client = reqwest::Client::new();

    let Ok(access_token_response) = client
        .post(format!(
            "https://github.com/login/oauth/access_token?client_id={client_id}&client_secret={client_secret}&code={code}",
            client_id = github_config.client_id,
            client_secret = github_config.client_secret,
            code = code,
        ))
        .header("Accept", "application/json")
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
    else {
        return (
            StatusCode::BAD_REQUEST,
            Html(String::from("Failed to send `access_token` request")),
        )
            .into_response();
    };
    let Ok(access_token_response) = access_token_response.json::<AccessTokenResponse>().await
    else {
        return (
            StatusCode::BAD_REQUEST,
            Html(String::from(
                "Failed to parse JSON from `access_token` request",
            )),
        )
            .into_response();
    };

    let access_token = access_token_response.access_token;

    let Ok(user_data_response) = client
        .get("https://api.github.com/user")
        .header("User-Agent", "oct")
        .header("Accept", "application/json")
        .header("Authorization", format!("Bearer {access_token}"))
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
    else {
        return (
            StatusCode::BAD_REQUEST,
            Html(String::from("Failed to sent `user` request")),
        )
            .into_response();
    };
    let Ok(user_data_response) = user_data_response.json::<UserDataResponse>().await else {
        return (
            StatusCode::BAD_REQUEST,
            Html(String::from("Failed to parse JSON from `user` request")),
        )
            .into_response();
    };

    let user = User {
        access_token,
        login: user_data_response.login,
    };
    let Ok(()) = user.save() else {
        return (
            StatusCode::BAD_REQUEST,
            Html(String::from("Failed to `user.json`")),
        )
            .into_response();
    };

    Redirect::permanent("/repos").into_response()
}
