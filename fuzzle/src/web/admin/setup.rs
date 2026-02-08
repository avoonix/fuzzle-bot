use std::{pin::Pin, sync::Arc};

use actix_files::Files;
use actix_web::{App, HttpServer, body::{BoxBody, EitherBody}, dev::{Service, ServiceRequest, ServiceResponse}, http::header, middleware, web::{self, route}};

use crate::{
    Config, background_tasks::{TagManagerService, TfIdfService}, bot::Bot, database::Database, qdrant::VectorDatabase, services::Services, web::{server::page, shared::AppState}
};

use super::service;

// // The function signature must match the internal middleware requirements
// async fn cors_middleware<S, B>(
//     req: ServiceRequest,
//     srv: &S,
// ) -> Result<ServiceResponse<B>, actix_web::Error>
// where
//     S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error>,
// {
//     let mut res = srv.call(req).await?;
//     res.headers_mut().insert(
//         actix_web::http::header::ACCESS_CONTROL_ALLOW_ORIGIN,
//         actix_web::http::header::HeaderValue::from_static("*"),
//     );
//     Ok(res)
// }

fn cors_middleware<S, B>(
    req: ServiceRequest,
    srv: &S,
) -> Pin<Box<dyn Future<Output = Result<ServiceResponse<EitherBody<B, BoxBody>>, actix_web::Error>>>>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error>,
    S::Future: 'static,
    B: 'static,
{
    // 1. Handle Preflight (OPTIONS)
    if req.method() == actix_web::http::Method::OPTIONS {
        let res = ServiceResponse::new(
            req.into_parts().0,
            actix_web::HttpResponse::NoContent()
                .insert_header((header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"))
                .insert_header((header::ACCESS_CONTROL_ALLOW_METHODS, "GET, POST, PUT, DELETE, OPTIONS"))
                .insert_header((header::ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type, Authorization"))
                .finish(),
        )
        // Wrap this in the "Right" variant of EitherBody
        .map_into_right_body();

        return Box::pin(async move { Ok(res) });
    }

    // 2. Handle Normal Requests
    let fut = srv.call(req);
    Box::pin(async move {
        let mut res = fut.await?;
        
        res.headers_mut().insert(
            header::ACCESS_CONTROL_ALLOW_ORIGIN,
            header::HeaderValue::from_static("*"),
        );
        
        // Wrap this in the "Left" variant to match the return type
        Ok(res.map_into_left_body())
    })
}


pub fn setup_admin_server(
    config: Arc<Config>,
    database: Database,
    tag_manager: TagManagerService,
    bot: Bot,
    tfidf_service: TfIdfService,
    // tag_worker: TagWorker,
    vector_db: VectorDatabase,
    services: Services,
) {
    tokio::spawn(async move {
        let addr = config.admin_http_listen_address.clone();
        tracing::info!("admin server listening on http://{}", addr);
        HttpServer::new(move || {
            App::new()
                .wrap(tracing_actix_web::TracingLogger::default())
                .wrap_fn(cors_middleware)

                .app_data(web::Data::new(AppState {
                    config: config.clone(),
                    database: database.clone(),
                    tag_manager: tag_manager.clone(),
                    bot: bot.clone(),
                    tfidf_service: tfidf_service.clone(),
                    // tag_worker: tag_worker.clone(),
                    vector_db: vector_db.clone(),
                    services: services.clone(),
                }))
                .service(service::get_pending_sets)
                .service(service::ban_set)
                .service(service::unban_set)
                .service(service::get_similar_stickers)
                .service(service::ban_sticker)
                .service(service::unban_sticker)
                .service(service::get_stickers_in_set)
                .service(service::approve_set)
                .service(service::scan_all_stickers_for_bans)
                .service(service::get_banned_stickers)
                .service(service::get_banned_sticker_thumbnail)
                .wrap(middleware::Compress::default())
        })
        .bind(addr)
        .expect("address not to be in use")
        .run()
        .await
        .expect("server not to die");
    });
}
