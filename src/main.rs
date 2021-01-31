mod juniper_hyper;

use std::sync::Arc;

use hyper::{
    service::{make_service_fn, service_fn},
    Body, Method, Response, Server, StatusCode,
};
use juniper::{EmptyMutation, EmptySubscription, FieldError, FieldResult, RootNode};

#[tokio::main]
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
                        (&Method::GET, "/") => juniper_hyper::graphiql("/graphql", None).await,
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

    // Ok(())
}

pub type Schema = RootNode<'static, Query, EmptyMutation<Context>, EmptySubscription<Context>>;

pub fn create_schema() -> Schema {
    Schema::new(Query, Default::default(), Default::default())
}

#[derive(Default)]
pub struct Query;

pub struct Context {}

impl juniper::Context for Context {} 

#[juniper::graphql_object(context = Context)]
impl Query {
    pub fn feature(_context: &Context, input: String) -> Result<gherkin_rust::Feature, FieldError> {
        gherkin_rust::Feature::parse(&input).map_err(|e| e.into())
    }
}
