use rusty_v8 as v8;

use bytes::Bytes;
use lazy_static::lazy_static;
use log::info;
use std::collections::BTreeSet;
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
    static ref FS: BTreeSet<String> = vec!["a", "b", "c"].into_iter().map(String::from).collect();
}
async fn exec_script(script: Bytes) -> Result<impl warp::Reply, Infallible> {
    let isolate = &mut v8::Isolate::new(Default::default());

    let scope = &mut v8::HandleScope::new(isolate);
    let context = v8::Context::new(scope);
    let scope = &mut v8::ContextScope::new(scope, context);

    let my_fn = v8::FunctionTemplate::new(
        scope,
        |scope: &mut v8::HandleScope, _: v8::FunctionCallbackArguments, mut rv: v8::ReturnValue| {
            let count = COUNTER.fetch_add(1, Ordering::SeqCst);
            rv.set(v8::Integer::new(scope, count).into());
        },
    );
    let my_fn_name = v8::String::new(scope, "counter").unwrap();
    let my_fn_impl = my_fn.get_function(scope).unwrap();

    let fs_fn = v8::FunctionTemplate::new(
        scope,
        |scope: &mut v8::HandleScope, _: v8::FunctionCallbackArguments, mut rv: v8::ReturnValue| {
            let a: &String = FS.iter().next().unwrap();
            rv.set(v8::String::new(scope, a).unwrap().into());
        },
    );
    let fs_fn_name = v8::String::new(scope, "fs").unwrap();
    let fs_fn_impl = fs_fn.get_function(scope).unwrap();

    let global = context.global(scope);
    global.set(scope, my_fn_name.into(), my_fn_impl.into());
    global.set(scope, fs_fn_name.into(), fs_fn_impl.into());

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
