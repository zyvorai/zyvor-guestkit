// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

use axum::Json;
use chrono::Utc;
use crate::models::ApiResponse;

pub async fn health() -> Json<ApiResponse<serde_json::Value>> {
    Json(ApiResponse::ok(serde_json::json!({
        "status": "healthy",
        "service": "zyvor-api",
        "timestamp": Utc::now().to_rfc3339(),
    })))
}
