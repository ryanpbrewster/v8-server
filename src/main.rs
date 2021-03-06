use rusty_v8 as v8;

use bytes::Bytes;
use lazy_static::lazy_static;
use log::{info, trace, warn};
use std::collections::BTreeMap;
use std::convert::Infallible;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use std::{ops::Bound, sync::Mutex};
use structopt::StructOpt;
use warp::Filter;

static QPS: AtomicUsize = AtomicUsize::new(0);

#[tokio::main]
async fn main() {
    env_logger::Builder::from_default_env()
        .format_timestamp_micros()
        .init();
    let _opts = Opts::from_args();

    let platform = v8::new_default_platform().unwrap();
    v8::V8::initialize_platform(platform);
    v8::V8::initialize();

    let hello = warp::get().and(warp::path::end()).map(|| "ok");
    let exec = warp::any()
        .and(warp::post())
        .and(warp::body::bytes())
        .and_then(exec_script);
    let routes = hello.or(exec);

    tokio::spawn(async {
        loop {
            info!("qps: {}", QPS.swap(0, Ordering::SeqCst));
            tokio::time::delay_for(Duration::from_millis(1_000)).await;
        }
    });

    warp::serve(routes).run(([127, 0, 0, 1], 9000)).await;
}

lazy_static! {
    static ref KV: Mutex<BTreeMap<String, String>> = Mutex::new(BTreeMap::new());
}
async fn exec_script(script: Bytes) -> Result<impl warp::Reply, Infallible> {
    Ok(tokio::task::spawn_blocking(move || {
        let isolate = &mut v8::Isolate::new(Default::default());
        let handle = isolate.thread_safe_handle();

        let scope = &mut v8::HandleScope::new(isolate);
        let context = v8::Context::new(scope);
        let scope = &mut v8::ContextScope::new(scope, context);

        let api_template = v8::ObjectTemplate::new(scope);

        let next_fn = v8::FunctionTemplate::new(
            scope,
            |scope: &mut v8::HandleScope,
             args: v8::FunctionCallbackArguments,
             mut rv: v8::ReturnValue| {
                let arg = args.get(0);
                let key = if arg.is_null_or_undefined() {
                    String::new()
                } else {
                    arg.to_string(scope).unwrap().to_rust_string_lossy(scope)
                };
                if let Some((next_key, _)) = KV
                    .lock()
                    .unwrap()
                    .range((Bound::Excluded(key), Bound::Unbounded))
                    .next()
                {
                    rv.set(v8::String::new(scope, next_key).unwrap().into());
                }
            },
        );
        let next_fn_name = v8::String::new(scope, "next").unwrap();
        api_template.set(next_fn_name.into(), next_fn.into());

        let get_fn = v8::FunctionTemplate::new(
            scope,
            |scope: &mut v8::HandleScope,
             args: v8::FunctionCallbackArguments,
             mut rv: v8::ReturnValue| {
                let key = args
                    .get(0)
                    .to_string(scope)
                    .unwrap()
                    .to_rust_string_lossy(scope);
                if let Some(value) = KV.lock().unwrap().get(&key) {
                    rv.set(v8::String::new(scope, value).unwrap().into());
                }
            },
        );
        let get_fn_name = v8::String::new(scope, "get").unwrap();
        api_template.set(get_fn_name.into(), get_fn.into());

        let set_fn = v8::FunctionTemplate::new(
            scope,
            |scope: &mut v8::HandleScope,
             args: v8::FunctionCallbackArguments,
             mut rv: v8::ReturnValue| {
                let key = args
                    .get(0)
                    .to_string(scope)
                    .unwrap()
                    .to_rust_string_lossy(scope);
                let value = args
                    .get(1)
                    .to_string(scope)
                    .unwrap()
                    .to_rust_string_lossy(scope);
                if let Some(prev) = KV.lock().unwrap().insert(key, value) {
                    rv.set(v8::String::new(scope, &prev).unwrap().into());
                }
            },
        );
        let set_fn_name = v8::String::new(scope, "set").unwrap();
        api_template.set(set_fn_name.into(), set_fn.into());

        let global = context.global(scope);
        let api_instance = api_template.new_instance(scope).unwrap();
        let api_name = v8::String::new(scope, "api").unwrap();
        global.set(scope, api_name.into(), api_instance.into());

        let code = std::str::from_utf8(script.as_ref()).unwrap();
        let code = v8::String::new(scope, &code).unwrap();
        let script = v8::Script::compile(scope, code, None).unwrap();
        let start = Instant::now();
        tokio::spawn(async move {
            tokio::time::delay_for(std::time::Duration::from_millis(50)).await;
            if handle.terminate_execution() {
                warn!("killing script after 50ms");
            }
        });

        let output = script.run(scope).unwrap();
        let result = output.to_string(scope).unwrap().to_rust_string_lossy(scope);
        QPS.fetch_add(1, Ordering::SeqCst);
        trace!("[{:?}] result: {}", start.elapsed(), result);
        result
    })
    .await
    .unwrap())
}

#[derive(StructOpt)]
struct Opts {}
