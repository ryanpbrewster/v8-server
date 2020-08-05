use rusty_v8 as v8;

use bytes::Bytes;
use log::info;
use serde::Serialize;
use std::convert::Infallible;
use std::sync::Arc;
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

    let state = Arc::new(State { seed: opts.seed });

    let hello = warp::get().and(warp::path::end()).map(|| "ok");
    let exec = {
        let state = state.clone();
        warp::any()
            .map(move || state.clone())
            .and(warp::post())
            .and(warp::body::bytes())
            .and_then(exec_script)
    };
    let routes = hello.or(exec);
    warp::serve(routes).run(([127, 0, 0, 1], 9000)).await;
}

async fn exec_script(state: Arc<State>, script: Bytes) -> Result<impl warp::Reply, Infallible> {
    let isolate = &mut v8::Isolate::new(Default::default());

    let scope = &mut v8::HandleScope::new(isolate);
    let context = v8::Context::new(scope);
    let scope = &mut v8::ContextScope::new(scope, context);

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

struct State {
    seed: u64,
}
