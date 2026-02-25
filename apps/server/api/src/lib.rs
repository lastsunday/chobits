pub mod asr;
pub mod auth;
pub mod auth_error;
pub mod common;
pub mod config;
pub mod i18n;
pub mod index;
pub mod llm;
pub mod matrix;
pub mod mcp;
pub mod ota;
pub mod ota_data;
pub mod ota_error;
pub mod server;
pub mod tts;
pub mod util;
pub mod vad;
pub mod ws;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::ServiceExt;
use axum::extract::DefaultBodyLimit;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::routing::get;
use bytesize::ByteSize;
use either::Either;
use framework::config::auth::AuthConfig;
use framework::error::ApiError;
use framework::error::ApiResult;
use framework::trace::LatencyOnResponse;
use futures::future::join_all;
use migration::MigratorTrait;
use sea_orm::DatabaseConnection;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tower_http::compression::CompressionLayer;
use tower_http::cors;
use tower_http::cors::CorsLayer;
use tower_http::normalize_path::NormalizePathLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use tower_layer::Layer;
use utoipa::OpenApi;
use utoipa::openapi::security::Http;
use utoipa::openapi::security::HttpAuthScheme;
use utoipa::openapi::security::SecurityScheme;
use utoipa_axum::router::OpenApiRouter;
use utoipa_scalar::{Scalar, Servable as ScalarServable};

use framework::auth::Jwt;

use crate::asr::AsrFactory;
use crate::config::Config;
use crate::config::asr::AsrConfig;
use crate::config::audio::AudioConfig;
use crate::config::database::DatabaseConfig;
use crate::config::llm::LlmConfig;
use crate::config::matrix::MatrixConfig;
use crate::config::mcp::McpConfig;
use crate::config::server::ServerConfig;
use crate::config::session::SessionConfig;
use crate::config::tts::TtsConfig;
use crate::config::vad::VadConfig;
use crate::config::ws::WsConfig;
use crate::llm::LlmFactory;
use crate::tts::TtsFactory;
use crate::vad::VadFactory;

#[macro_use]
extern crate rust_i18n;
i18n!("locales", fallback = "zh");

#[allow(clippy::too_many_arguments)]
pub async fn start(
    server_config: Arc<ServerConfig>,
    database_config: Arc<DatabaseConfig>,
    session_config: Arc<SessionConfig>,
    mcp_config: Arc<McpConfig>,
    vad_config: Arc<VadConfig>,
    audio_config: Arc<AudioConfig>,
    auth_config: Arc<AuthConfig>,
    ws_config: Arc<WsConfig>,
    tts_config: Arc<TtsConfig>,
    asr_config: Arc<AsrConfig>,
    llm_config: Arc<LlmConfig>,
    matrix_config: Arc<MatrixConfig>,
) -> anyhow::Result<()> {
    // auth
    Jwt::init(auth_config.clone());
    // database init
    let database_url = database_config.url.as_ref().expect("database url is empty");
    let conn: sea_orm::DatabaseConnection =
        framework::database::establish_connection(database_url).await?;
    conn.ping().await?;
    tracing::info!("Database connected successfully");
    // database schema init or upgrade
    migration::Migrator::up(&conn, None).await?;
    tracing::info!("init tts factory");
    TtsFactory::init(tts_config, audio_config.clone()).await?;
    tracing::info!("init tts factory successfully");
    tracing::info!("init vad factory");
    VadFactory::init(vad_config.clone()).await;
    tracing::info!("init vad factory successfully");
    tracing::info!("init asr factory");
    AsrFactory::init(asr_config).await;
    tracing::info!("init asr factor3y successfully");
    tracing::info!("init llm factory");
    LlmFactory::init(llm_config).await;
    tracing::info!("init llm factory successfully");
    let ct = tokio_util::sync::CancellationToken::new();
    let ct_for_app = ct.clone();
    let mut handles = Vec::new();
    let session_config_clone = session_config.clone();
    let mcp_config_clone = mcp_config.clone();
    let vad_config_clone = vad_config.clone();
    let audio_config_clone = audio_config.clone();
    let auth_config_clone = auth_config.clone();
    let ws_config_clone = ws_config.clone();
    handles.push(tokio::spawn(async move {
        if let Err(error) = start_app(
            server_config,
            session_config_clone,
            mcp_config_clone,
            vad_config_clone,
            audio_config_clone,
            auth_config_clone,
            ws_config_clone,
            conn,
            ct_for_app,
        )
        .await
        {
            tracing::error!("{:?}", error);
        }
    }));
    if matrix_config.enable.expect("matrix enable is empty") {
        handles.push(tokio::spawn(async move {
            if let Err(error) = start_matrix_client(
                matrix_config,
                session_config,
                mcp_config,
                vad_config,
                audio_config,
            )
            .await
            {
                tracing::error!("{:?}", error);
            }
        }));
    }
    let join_results = join_all(handles).await;
    tracing::info!("all joinhandle({}) end", join_results.len());
    Ok(())
}

pub async fn start_matrix_client(
    matrix_config: Arc<MatrixConfig>,
    session_config: Arc<SessionConfig>,
    mcp_config: Arc<McpConfig>,
    vad_config: Arc<VadConfig>,
    audio_config: Arc<AudioConfig>,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("matrix client start");
    matrix::client::start(
        matrix_config,
        session_config,
        mcp_config,
        vad_config,
        audio_config,
    )
    .await?;
    tracing::info!("matrix client end");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn start_app(
    server_config: Arc<ServerConfig>,
    session_config: Arc<SessionConfig>,
    mcp_config: Arc<McpConfig>,
    vad_config: Arc<VadConfig>,
    audio_config: Arc<AudioConfig>,
    auth_config: Arc<AuthConfig>,
    ws_config: Arc<WsConfig>,
    conn: sea_orm::DatabaseConnection,
    ct: CancellationToken,
) -> anyhow::Result<()> {
    let addrs = server_config
        .address
        .as_ref()
        .expect("server address is empty")
        .addrs
        .clone();
    let port = match &server_config
        .port
        .as_ref()
        .expect("server port is empty")
        .ports
    {
        Either::Left(value) => value,
        Either::Right(values) => values.first().expect("port is empty"),
    };
    // state
    let state = AppState {
        conn,
        session_config,
        mcp_config,
        vad_config,
        audio_config,
        auth_config,
        ws_config,
    };
    // router
    let (app, ct) = create_router(state, ct);
    // app start
    tracing::info!("app start");
    let addr = match addrs {
        Either::Left(value) => value.to_string(),
        Either::Right(values) => values.first().expect("addrs is empty").to_string(),
    };
    let listener = TcpListener::bind(format!("{addr}:{port}")).await?;
    tracing::info!("listening on {addr}:{port}");
    let app = NormalizePathLayer::trim_trailing_slash().layer(app);
    axum::serve(
        listener,
        ServiceExt::<Request>::into_make_service_with_connect_info::<SocketAddr>(app),
    )
    .with_graceful_shutdown(async move {
        tokio::signal::ctrl_c().await.unwrap();
        ct.cancel();
    })
    .await?;
    tracing::info!("app end");
    Ok(())
}

#[derive(OpenApi)]
#[openapi()]
struct ApiDoc;

pub fn create_router(
    state: AppState,
    cancellation_token: CancellationToken,
) -> (Router, CancellationToken) {
    let mut api = ApiDoc::openapi();
    api.components.as_mut().unwrap().add_security_scheme(
        "AccessToken",
        SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
    );
    let mut api_router = OpenApiRouter::with_openapi(api);
    api_router = setup_index(api_router);
    api_router = setup_auth(api_router, state.clone());
    api_router = setup_ota(api_router, state.clone());
    api_router = setup_ws(api_router, state.clone());
    api_router = setup_mcp(api_router, state.clone(), cancellation_token.child_token());
    let (mut app, api) = api_router.split_for_parts();
    app = setup_web(app);
    app = setup_api_fallback(app);
    app = setup_default(app);
    app = app.merge(Scalar::with_url("/docs", api));
    (app, cancellation_token)
}

pub fn setup_default(router: Router) -> Router {
    let app = router
        .fallback(web::index_handler)
        .method_not_allowed_fallback(async || -> ApiResult<()> {
            tracing::warn!("Method not allowed");
            Err(ApiError::MethodNotAllowed)
        });
    let timeout =
        TimeoutLayer::with_status_code(StatusCode::REQUEST_TIMEOUT, Duration::from_secs(120));
    let body_limit = DefaultBodyLimit::max(ByteSize::mib(10).as_u64() as usize);
    let cors = CorsLayer::new()
        .allow_origin(cors::Any)
        .allow_methods(cors::Any)
        .allow_headers(cors::Any)
        .allow_credentials(false)
        .max_age(Duration::from_secs(3600 * 12));
    let tracing = TraceLayer::new_for_http()
        .make_span_with(|request: &Request| {
            let method = request.method();
            let path = request.uri().path();
            let id = xid::new();
            tracing::info_span!("Api Request",id = %id,method = %method,path = %path)
        })
        .on_request(())
        .on_failure(())
        .on_response(LatencyOnResponse);
    app.layer(timeout)
        .layer(body_limit)
        .layer(tracing)
        .layer(cors)
}

pub fn setup_index(router: OpenApiRouter) -> OpenApiRouter {
    router.merge(index::create_routes())
}

pub fn setup_ws(router: OpenApiRouter, state: AppState) -> OpenApiRouter {
    router.merge(ws::create_routes(state))
}

pub fn setup_mcp(
    router: OpenApiRouter,
    state: AppState,
    cancellation_token: CancellationToken,
) -> OpenApiRouter {
    router.merge(mcp::create_routes(state, cancellation_token))
}

pub fn setup_auth(router: OpenApiRouter, state: AppState) -> OpenApiRouter {
    api_setup(router, auth::create_routes(state))
}

pub fn setup_ota(router: OpenApiRouter, state: AppState) -> OpenApiRouter {
    api_setup(router, ota::create_routes(state))
}

fn api_setup(router: OpenApiRouter, api_router: OpenApiRouter) -> OpenApiRouter {
    router.nest("/api", api_router)
}

fn setup_api_fallback(router: Router) -> Router {
    router.nest(
        "/api",
        Router::new().fallback(async || -> ApiResult<()> {
            tracing::warn!("Not found");
            Err(ApiError::NotFound)
        }),
    )
}

pub fn setup_web(router: Router) -> Router {
    router
        .nest(
            "/assets",
            Router::new()
                .route("/{*file}", get(web::assets_handler))
                .route_layer(CompressionLayer::new()),
        )
        .nest(
            "/device/assets",
            Router::new()
                .route("/{*file}", get(web::device_assets_handler))
                .route_layer(CompressionLayer::new()),
        )
        .nest(
            "/test",
            Router::new()
                .route("/{*file}", get(web::test_handler))
                .route_layer(CompressionLayer::new()),
        )
}

#[derive(Clone, Debug, Default)]
pub struct AppState {
    pub conn: DatabaseConnection,
    pub session_config: Arc<SessionConfig>,
    pub mcp_config: Arc<McpConfig>,
    pub vad_config: Arc<VadConfig>,
    pub audio_config: Arc<AudioConfig>,
    pub auth_config: Arc<AuthConfig>,
    pub ws_config: Arc<WsConfig>,
}
