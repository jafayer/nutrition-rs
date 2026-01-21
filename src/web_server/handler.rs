use crate::ast::ast::Item;

use axum::{
    body::Body,
    extract::Json,
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Router,
};
use std::net::SocketAddr;

pub async fn to_item(Json(item): Json<Item>) -> impl IntoResponse {
    let output = match item {
        Item::Property(prop) => prop.to_string(),
        Item::Ingredient(ingredient) => ingredient.to_string(),
        Item::Recipe(recipe) => recipe.to_string(),
        Item::Exercise(exercise) => exercise.to_string(),
        Item::Day(day) => day.to_string(),
        Item::Ate(ate) => {
            format!("@ate \"{}\"({})", ate.food_alias, ate.quantity.to_string())
        }
        Item::Exercised(exercised) => {
            format!("@exercised \"{}\"({})", exercised.exercise_alias, exercised.quantity.to_string())
        }
        Item::Comment(comment) => comment,
    };
    
    (StatusCode::OK, output)
}

pub async fn run_server(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let app = Router::new()
        .route("/", post(handler));

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
     
    axum::serve(listener, app).await?;
    Ok(())
}

async fn handler(body: Body) -> impl IntoResponse {
    // convert request to Item and emit
    let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
    let json = String::from_utf8(bytes.to_vec()).unwrap();
    let item: Item = serde_json::from_str(&json).unwrap();
    to_item(Json(item)).await
}
