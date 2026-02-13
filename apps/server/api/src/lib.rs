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
use crate::config::llm::LlmConfig;
use crate::config::matrix::MatrixConfig;
use crate::config::tts::TtsConfig;
use crate::config::vad::VadConfig;
use crate::llm::LlmFactory;
use crate::tts::TtsFactory;
use crate::vad::VadFactory;

#[macro_use]
extern crate rust_i18n;
i18n!("locales", fallback = "zh");

pub async fn start(config: Arc<Config>) -> anyhow::Result<()> {
    let config_clone_for_app = config.clone();
    // auth
    Jwt::init(AuthConfig {
        access_token_secret: config.auth_access_token_secret.clone(),
        access_token_expires_in: config.auth_access_token_expires_in,
        refresh_token_secret: config.auth_refresh_token_secret.clone(),
        refresh_token_expires_in: config.auth_refresh_token_expires_in,
        audience: config.auth_audience.clone(),
        issuer: config.auth_issuer.clone(),
        client_id: config.auth_client_id.clone(),
        client_secret: config.auth_client_secret.clone(),
    });
    // database init
    let database_url = config.database_url.as_ref().expect("database url is empty");
    let conn: sea_orm::DatabaseConnection =
        framework::database::establish_connection(database_url).await?;
    conn.ping().await?;
    let conn_for_app = conn.clone();
    tracing::info!("Database connected successfully");
    // database schema init or upgrade
    migration::Migrator::up(&conn, None).await?;
    tracing::info!("init tts factory");
    let tts_config = TtsConfig {
        model: config.tts_model.clone(),
        path: config.tts_path.clone(),
        reference_prompt_text: config.tts_reference_prompt_text.clone(),
        reference_prompt_wav_path: config.tts_reference_prompt_wav_path.clone(),
    };
    let audio_config = AudioConfig {
        input_sample_rate: config.audio_input_sample_rate,
        input_frame_duration: config.audio_input_frame_duration,
        input_channel: config.audio_input_channel,
        output_sample_rate: config.audio_output_sample_rate,
        output_channel: config.audio_output_channel,
        output_frame_duration: config.audio_output_frame_duration,
    };
    TtsFactory::init(tts_config, audio_config).await?;
    tracing::info!("init tts factory successfully");
    tracing::info!("init vad factory");
    VadFactory::init(VadConfig {
        path: config.vad_path.clone(),
        num_threads: config.vad_num_threads,
    })
    .await;
    tracing::info!("init vad factory successfully");
    tracing::info!("init asr factory");
    AsrFactory::init(AsrConfig {
        path: config.asr_path.clone(),
    })
    .await;
    tracing::info!("init asr factor3y successfully");
    tracing::info!("init llm factory");
    LlmFactory::init(LlmConfig {
        model: config.llm_model.clone(),
        path: config.llm_path.clone(),
    })
    .await;
    tracing::info!("init llm factory successfully");
    let ct = tokio_util::sync::CancellationToken::new();
    let ct_for_app = ct.clone();
    let mut handles = Vec::new();
    handles.push(tokio::spawn(async move {
        if let Err(error) = start_app(config_clone_for_app, conn_for_app, ct_for_app).await {
            tracing::error!("{:?}", error);
        }
    }));
    if config.matrix_enable.expect("matrix enable is empty") {
        handles.push(tokio::spawn(async move {
            if let Err(error) = start_matrix_client(MatrixConfig {
                enable: config.matrix_enable,
                client_name: config.matrix_client_name.clone(),
                homeserver: config.matrix_homeserver.clone(),
                client_username: config.matrix_client_username.clone(),
                client_password: config.matrix_client_password.clone(),
            })
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

pub async fn start_matrix_client(config: MatrixConfig) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("matrix client start");
    matrix::client::start(config).await?;
    tracing::info!("matrix client end");
    Ok(())
}

pub async fn start_app(
    config: Arc<Config>,
    conn: sea_orm::DatabaseConnection,
    ct: CancellationToken,
) -> anyhow::Result<()> {
    let addrs = config.clone().address.addrs.clone();
    let port: u16 = config.server_port.expect("server port is empty");
    // state
    let state = AppState { conn, config };
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

#[derive(Clone, Debug)]
pub struct AppState {
    pub conn: DatabaseConnection,
    pub config: Arc<Config>,
}
