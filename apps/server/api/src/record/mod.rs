pub mod collector;
pub mod observer;
pub mod wav;

use axum::{
    debug_handler,
    extract::{Path, Query, State},
    response::IntoResponse,
};
use framework::{data::ApiResponse, error::AppResult, middleware::get_auth_layer};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QueryTrait};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::AppState;
use entity::prelude::*;
use entity::{frame, round, round_data};

const TAG: &str = "record";

pub fn create_routes(state: AppState) -> OpenApiRouter {
    OpenApiRouter::new()
        .routes(routes!(list_rounds))
        .routes(routes!(get_round))
        .routes(routes!(list_round_data))
        .routes(routes!(get_round_data_blob))
        .routes(routes!(list_frames))
        .route_layer(get_auth_layer())
        .with_state(state)
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListRoundsParams {
    pub user_id: Option<String>,
    pub page: Option<u64>,
    pub page_size: Option<u64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RoundListResponse {
    pub items: Vec<round::Model>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
}

#[debug_handler]
#[utoipa::path(get, path = "/record/rounds", tag = TAG, params(ListRoundsParams), responses(
    (status = OK, body = ApiResponse<RoundListResponse>)
))]
async fn list_rounds(
    State(AppState { conn, .. }): State<AppState>,
    Query(params): Query<ListRoundsParams>,
) -> AppResult<ApiResponse<RoundListResponse>> {
    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(20).min(100);
    let paginator = Round::find()
        .order_by_desc(round::Column::CreateDatetime)
        .apply_if(params.user_id, |query, uid| {
            query.filter(round::Column::UserId.eq(uid))
        })
        .paginate(&conn, page_size);
    let items = paginator.fetch_page(page - 1).await?;
    let total = paginator.num_items().await?;
    Ok(ApiResponse::success(Some(RoundListResponse {
        items,
        total,
        page,
        page_size,
    })))
}

#[debug_handler]
#[utoipa::path(get, path = "/record/rounds/{id}", tag = TAG, responses(
    (status = OK, body = ApiResponse<round::Model>)
))]
async fn get_round(
    State(AppState { conn, .. }): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<ApiResponse<round::Model>> {
    let item = Round::find_by_id(&id).one(&conn).await?.ok_or_else(|| {
        framework::err!(framework::error::critical_code::CriticalErrorCode::ResourceNotFound)
    })?;
    Ok(ApiResponse::success(Some(item)))
}

#[debug_handler]
#[utoipa::path(get, path = "/record/rounds/{id}/data", tag = TAG, responses(
    (status = OK, body = ApiResponse<Vec<round_data::Model>>)
))]
async fn list_round_data(
    State(AppState { conn, .. }): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<ApiResponse<Vec<round_data::Model>>> {
    let items = RoundData::find()
        .filter(round_data::Column::RoundId.eq(&id))
        .all(&conn)
        .await?;
    Ok(ApiResponse::success(Some(items)))
}

#[debug_handler]
#[utoipa::path(get, path = "/record/rounds/{id}/data/{data_id}/blob", tag = TAG, responses(
    (status = 200, content_type = "audio/wav", body = Vec<u8>),
))]
async fn get_round_data_blob(
    State(AppState { conn, .. }): State<AppState>,
    Path((_id, data_id)): Path<(String, String)>,
) -> AppResult<axum::response::Response> {
    let item = RoundData::find_by_id(&data_id)
        .one(&conn)
        .await?
        .ok_or_else(|| {
            framework::err!(framework::error::critical_code::CriticalErrorCode::ResourceNotFound)
        })?;
    match item.data {
        Some(bytes) => Ok(([("Content-Type", "audio/wav")], bytes).into_response()),
        None => Err(framework::err!(
            framework::error::critical_code::CriticalErrorCode::ResourceNotFound
        )),
    }
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListFramesParams {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrameListResponse {
    pub items: Vec<frame::Model>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
}

#[debug_handler]
#[utoipa::path(get, path = "/record/rounds/{id}/frames", tag = TAG, params(ListFramesParams), responses(
    (status = OK, body = ApiResponse<FrameListResponse>)
))]
async fn list_frames(
    State(AppState { conn, .. }): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<ListFramesParams>,
) -> AppResult<ApiResponse<FrameListResponse>> {
    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(50).min(500);
    let paginator = Frame::find()
        .filter(frame::Column::RoundId.eq(&id))
        .order_by_asc(frame::Column::Seq)
        .paginate(&conn, page_size);
    let items = paginator.fetch_page(page - 1).await?;
    let total = paginator.num_items().await?;
    Ok(ApiResponse::success(Some(FrameListResponse {
        items,
        total,
        page,
        page_size,
    })))
}
