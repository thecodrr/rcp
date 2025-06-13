use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, Result};
use env_logger::Builder;
use log::{info, warn, LevelFilter};
use reqwest::Client;
use std::env;

async fn cors_proxy(req: HttpRequest, body: web::Bytes) -> Result<HttpResponse> {
    let url = match req.match_info().get("url") {
        Some(url) => {
            // Basic URL validation
            if url.contains("://") && !url.starts_with("http://") && !url.starts_with("https://") {
                return {
                    warn!("Bad request: unsupported protocol");
                    Ok(HttpResponse::BadRequest().body("Unsupported protocol. Only HTTP and HTTPS are allowed."))
                };
            }

            // Ensure we have a domain name with at least one dot
            let domain = url.split("://").last().unwrap_or(url);
            if !domain.contains('.') {
                return {
                    warn!("Bad request: invalid domain - {}", url);
                    Ok(HttpResponse::BadRequest().body("Invalid domain name"))
                };
            }

            // Prepend https:// if no protocol is specified
            if !url.starts_with("http://") && !url.starts_with("https://") {
                format!("https://{}", url)
            } else {
                url.to_string()
            }
        },
        None => {
            return {
                warn!("Bad request: no url specified");
                Ok(HttpResponse::BadRequest().body("No URL specified"))
            }
        }
    };

    info!("Forwarding request to {}", url);

    let client = Client::new();

    // Determine the HTTP method
    let method = match *req.method() {
        actix_web::http::Method::GET => reqwest::Method::GET,
        actix_web::http::Method::POST => reqwest::Method::POST,
        actix_web::http::Method::PUT => reqwest::Method::PUT,
        actix_web::http::Method::DELETE => reqwest::Method::DELETE,
        _ => {
            return {
                warn!("Bad request: not valid HTTP method specified");
                Ok(HttpResponse::MethodNotAllowed().finish())
            }
        }
    };

    // Forward the request to the specified URL
    let response = match client
        .request(method, url.clone())
        .body(body.to_vec())
        .send()
        .await
    {
        Ok(response) => response,
        Err(e) => {
            warn!("Failed to forward request to {}: {}", url, e);
            return Ok(HttpResponse::BadGateway().body(format!("Failed to forward request: {}", e)));
        }
    };

    // Get the Content-Type header from the response
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .map(|header| header.to_str().unwrap())
        .unwrap_or("application/json")
        .to_string();

    // Get the response body
    let body = match response.bytes().await {
        Ok(bytes) => bytes,
        Err(e) => {
            warn!("Failed to read response body: {}", e);
            return Ok(HttpResponse::BadGateway().body(format!("Failed to read response body: {}", e)));
        }
    };

    // Create a new response with the response body and appropriate headers
    Ok(HttpResponse::Ok()
        .append_header(("Access-Control-Allow-Origin", "*"))
        .append_header((
            "Access-Control-Allow-Methods",
            "GET, POST, PUT, DELETE, OPTIONS",
        ))
        .append_header(("Access-Control-Allow-Headers", "Content-Type"))
        .append_header(("Access-Control-Max-Age", "3600"))
        .append_header(("Content-Type", content_type))
        .body(body))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Set up logger based on the environment variable
    let logging_enabled = env::var("LOGGING_ENABLED")
        .map(|val| val == "true")
        .unwrap_or(false);

    if logging_enabled {
        Builder::new().filter_level(LevelFilter::Info).init();
    }

    // Get the port from the environment variable or use the default value 8080
    let port = env::var("PORT")
        .map(|val| val.parse().unwrap_or(8080))
        .unwrap_or(8080);

    let address = env::var("ADDRESS")
        .unwrap_or("0.0.0.0".to_string())
        .to_string();

    HttpServer::new(|| {
        App::new().service(
            web::resource("/{url:.+}")
                .route(web::get().to(cors_proxy))
                .route(web::post().to(cors_proxy))
                .route(web::put().to(cors_proxy))
                .route(web::delete().to(cors_proxy)),
        )
    })
    .bind((address, port))?
    .run()
    .await
}
