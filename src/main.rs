mod juniper_graphql_ws;
mod juniper_hyper;
mod serve;

use std::{collections::HashMap, io::ErrorKind, pin::Pin, sync::Arc};

use async_stream::stream;
use futures::Stream;

use hyper::{
    service::{make_service_fn, service_fn},
    Body, Method, Response, Server, StatusCode,
};
use juniper::{EmptyMutation, FieldError, InputValue, RootNode};
use juniper_graphql_ws::ConnectionConfig;
use serve::is_websocket_upgrade;

#[tokio::main(flavor = "multi_thread", worker_threads = 20)]
async fn main() {
    tracing_subscriber::fmt::init();

    let context = Arc::new(Context {});
    let addr = ([127, 0, 0, 1], 3000).into();
    let schema = Arc::new(create_schema());

    let new_service = make_service_fn(move |_| {
        let schema = schema.clone();
        let cloned_context = context.clone();

        async {
            Ok::<_, hyper::Error>(service_fn(move |request| {
                let schema = schema.clone();
                let context = cloned_context.clone();
                async {
                    match (request.method(), request.uri().path()) {
                        (&Method::GET, "/") => {
                            juniper_hyper::playground("/graphql", Some("/graphql")).await
                        }
                        (&Method::GET, "/graphql") if is_websocket_upgrade(&request) => {
                            Ok(serve::serve_graphql_ws(
                                request,
                                schema,
                                |vars: HashMap<String, InputValue>| async move {
                                    let is_valid_token = vars
                                        .get("bearer")
                                        .and_then(|x| x.as_string_value())
                                        .map(|token| token == "test")
                                        .unwrap_or(false);

                                    if !is_valid_token {
                                        return Err(std::io::Error::new(
                                            ErrorKind::Other,
                                            "invalid token provided",
                                        ));
                                    }
                                    Ok::<_, std::io::Error>(
                                        ConnectionConfig::new(Context {}).with_keep_alive_interval(
                                            std::time::Duration::new(15, 0),
                                        ),
                                    )
                                },
                            ))
                        }
                        (&Method::GET, "/graphql") | (&Method::POST, "/graphql") => {
                            juniper_hyper::graphql(schema, context, request).await
                        }
                        _ => {
                            let mut response = Response::new(Body::empty());
                            *response.status_mut() = StatusCode::NOT_FOUND;
                            Ok(response)
                        }
                    }
                }
            }))
        }
    });

    let server = Server::bind(&addr).serve(new_service);
    tracing::info!("Listening on http://{}", addr);

    if let Err(e) = server.await {
        tracing::error!("server error: {}", e)
    }
}

pub type Schema = RootNode<'static, Query, EmptyMutation<Context>, Subscription>;

pub fn create_schema() -> Schema {
    Schema::new(Query, Default::default(), Subscription)
}

#[derive(Default)]
pub struct Query;

#[derive(Default)]
pub struct Subscription;

pub struct Context {}

impl juniper::Context for Context {}

#[juniper::graphql_object(context = Context)]
impl Query {
    pub fn feature(_context: &Context, input: String) -> Result<gherkin_rust::Feature, FieldError> {
        gherkin_rust::Feature::parse(&input).map_err(|e| e.into())
    }
}

type StringStream = Pin<Box<dyn Stream<Item = Result<String, FieldError>> + Send>>;

#[juniper::graphql_subscription(context = Context)]
impl Subscription {
    async fn hello_world() -> StringStream {
        let stream = stream! {
            yield Ok(String::from("Hello"));
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            yield Ok(String::from("World!"));
        };
        Box::pin(stream)
    }
}
