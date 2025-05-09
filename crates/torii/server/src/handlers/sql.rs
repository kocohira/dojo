use std::net::IpAddr;
use std::sync::Arc;

use http::header::CONTENT_TYPE;
use hyper::{Body, Method, Request, Response, StatusCode};
use include_str;
use sqlx::SqlitePool;
use torii_sqlite::utils::map_row_to_json;

use super::Handler;

#[derive(Debug)]
pub struct SqlHandler {
    pool: Arc<SqlitePool>,
}

impl SqlHandler {
    pub fn new(pool: Arc<SqlitePool>) -> Self {
        Self { pool }
    }

    pub async fn execute_query(&self, query: String) -> Response<Body> {
        match sqlx::query(&query).fetch_all(&*self.pool).await {
            Ok(rows) => {
                let result: Vec<_> = rows.iter().map(map_row_to_json).collect();
                let json = match serde_json::to_string(&result) {
                    Ok(json) => json,
                    Err(e) => {
                        return Response::builder()
                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                            .body(Body::from(format!("Failed to serialize result: {:?}", e)))
                            .unwrap();
                    }
                };

                Response::builder()
                    .status(StatusCode::OK)
                    .header(CONTENT_TYPE, "application/json")
                    .body(Body::from(json))
                    .unwrap()
            }
            Err(e) => Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from(format!("Query error: {:?}", e)))
                .unwrap(),
        }
    }

    async fn extract_query(&self, req: Request<Body>) -> Result<String, Response<Body>> {
        match *req.method() {
            Method::GET => {
                // Get the query from the query params
                let params = req.uri().query().unwrap_or_default();
                form_urlencoded::parse(params.as_bytes())
                    .find(|(key, _)| key == "q" || key == "query")
                    .map(|(_, value)| value.to_string())
                    .ok_or(
                        Response::builder()
                            .status(StatusCode::BAD_REQUEST)
                            .body(Body::from("Missing 'q' or 'query' parameter."))
                            .unwrap(),
                    )
            }
            Method::POST => {
                // Get the query from request body
                let body_bytes = hyper::body::to_bytes(req.into_body()).await.map_err(|_| {
                    Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .body(Body::from("Failed to read query from request body"))
                        .unwrap()
                })?;
                String::from_utf8(body_bytes.to_vec()).map_err(|_| {
                    Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .body(Body::from("Invalid query"))
                        .unwrap()
                })
            }
            _ => Err(Response::builder()
                .status(StatusCode::METHOD_NOT_ALLOWED)
                .body(Body::from("Only GET and POST methods are allowed"))
                .unwrap()),
        }
    }

    async fn serve_playground(&self) -> Response<Body> {
        let html = include_str!("../../static/sql-playground.html");

        Response::builder()
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, "text/html")
            .header("Access-Control-Allow-Origin", "*")
            .body(Body::from(html))
            .unwrap()
    }

    async fn handle_request(&self, req: Request<Body>) -> Response<Body> {
        if req.method() == Method::GET && req.uri().query().unwrap_or_default().is_empty() {
            self.serve_playground().await
        } else {
            match self.extract_query(req).await {
                Ok(query) => self.execute_query(query).await,
                Err(_) => self.serve_playground().await,
            }
        }
    }
}

#[async_trait::async_trait]
impl Handler for SqlHandler {
    fn should_handle(&self, req: &Request<Body>) -> bool {
        req.uri().path().starts_with("/sql")
    }

    async fn handle(&self, req: Request<Body>, _client_addr: IpAddr) -> Response<Body> {
        self.handle_request(req).await
    }
}
