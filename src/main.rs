use rusty_v8 as v8;

use bytes::Bytes;
use lazy_static::lazy_static;
use log::info;
use std::collections::BTreeMap;
use std::convert::Infallible;
use std::sync::atomic::{AtomicI32, Ordering};
use structopt::StructOpt;
use warp::Filter;

#[tokio::main]
async fn main() {
    env_logger::init();

    let opts = Opts::from_args();
    let seed = opts.seed;
    info!("starting with seed {}", seed);

    let platform = v8::new_default_platform().unwrap();
    v8::V8::initialize_platform(platform);
    v8::V8::initialize();

    let hello = warp::get().and(warp::path::end()).map(|| "ok");
    let exec = warp::any()
        .and(warp::post())
        .and(warp::body::bytes())
        .and_then(exec_script);
    let routes = hello.or(exec);
    warp::serve(routes).run(([127, 0, 0, 1], 9000)).await;
}

static COUNTER: AtomicI32 = AtomicI32::new(0);
lazy_static! {
    static ref KV: BTreeMap<String, String> = vec![("a", "Hello"), ("b", "Goodbye")]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
}
async fn exec_script(script: Bytes) -> Result<impl warp::Reply, Infallible> {
    let isolate = &mut v8::Isolate::new(Default::default());

    let scope = &mut v8::HandleScope::new(isolate);
    let context = v8::Context::new(scope);
    let scope = &mut v8::ContextScope::new(scope, context);

    let api_template = v8::ObjectTemplate::new(scope);

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
            let default = String::new();
            let value = KV.get(&key).unwrap_or(&default);
            rv.set(v8::String::new(scope, value).unwrap().into());
        },
    );
    let get_fn_name = v8::String::new(scope, "get").unwrap();
    api_template.set(get_fn_name.into(), get_fn.into());

    let set_fn = v8::FunctionTemplate::new(
        scope,
        |scope: &mut v8::HandleScope,
         args: v8::FunctionCallbackArguments,
         mut rv: v8::ReturnValue| {
            println!(
                "{:?}",
                args.get(0)
                    .to_string(scope)
                    .unwrap()
                    .to_rust_string_lossy(scope)
            );
            let a: &String = KV.keys().next().unwrap();
            rv.set(v8::String::new(scope, a).unwrap().into());
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
    println!("javascript code: {}", code.to_rust_string_lossy(scope));

    let script = v8::Script::compile(scope, code, None).unwrap();
    let result = script.run(scope).unwrap();
    let result = result.to_string(scope).unwrap();
    let result = result.to_rust_string_lossy(scope);
    println!("result: {}", result);
    Ok(result)
}

#[derive(StructOpt)]
struct Opts {
    #[structopt(long, default_value = "0")]
    seed: u64,
}
