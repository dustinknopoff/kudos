use tracing_subscriber::fmt::format::Pretty;
use tracing_subscriber::fmt::time::UtcTime;
use tracing_subscriber::prelude::*;
use tracing_web::{performance_layer, MakeConsoleWriter};

use worker::{kv::KvStore, *};

#[event(start)]
fn start() {
    let fmt_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_ansi(false) // Only partially supported across JavaScript runtimes
        .with_timer(UtcTime::rfc_3339()) // std::time is not available in browsers
        .with_writer(MakeConsoleWriter); // write events to the console
    let perf_layer = performance_layer().with_details_from_fields(Pretty::default());
    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(perf_layer)
        .init();
}

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    tracing::info!(request=?req, "Handling request");
    let router = Router::new();

    router
        .get_async("/", |req, ctx| async move {
            let key = match req.url()?.query_pairs().find(|(key, _)| key == "path") {
                Some((_, value)) => value.into_owned(),
                None => return Response::error("Need a path", 400)
            };
            let kudos_namespace = ctx.kv("KUDOS")?;
            let count = get_kudos(&kudos_namespace, &key).await?;
            Response::from_html(format!(
                "<a id='kudos' hx-post='https://kudos.knopoff.dev/kudo' hx-swap='outerHTML'>👋 {count}</a>"
            ))
            .map(|resp| add_cors(resp))
        })
        .post_async("/kudo", |mut req, ctx| async move {
            let form = req.form_data().await?;
            let key: String = match form.get("path") {
                Some(FormEntry::Field(val)) => val,
                _ => return Response::error("Need a path", 400)
            };
            let kudos_namespace = ctx.kv("KUDOS")?;
            let count = get_kudos(&kudos_namespace, &key).await?;
            let new_count = {
                let count: isize = count.parse().unwrap_or(0);
                count + 1
            };
            kudos_namespace
                .put(&key, new_count.to_string())
                .unwrap()
                .execute()
                .await?;
            Response::from_html(format!(
                "<div id='kudos'>👋 {new_count}</div>"
            ))
            .map(|resp| add_cors(resp))
        })
        .options("/*catchall", |_, _| {
            Response::ok("ok").map(|resp| add_cors(resp))
        })
        .run(req, env)
        .await
}

fn add_cors(mut resp: Response) -> Response {
    let headers = resp.headers_mut();
    headers.set("Access-Control-Allow-Origin", "*").unwrap();
    headers
        .set(
            "Access-Control-Allow-Methods",
            "GET,HEAD,POST,OPTIONS,PUT,DELETE",
        )
        .unwrap();
    headers.set("Content-Type", "text/html").unwrap();
    headers.set("Access-Control-Allow-Headers", "*").unwrap();
    headers.set("Access-Control-Expose-Headers", "*").unwrap();
    resp
}

async fn get_kudos(kudos_namespace: &KvStore, key: &'_ str) -> Result<String> {
    Ok(kudos_namespace
        .get(key)
        .text()
        .await?
        .unwrap_or_else(|| "Kudos".into()))
}
