# Logger Module Instruction

## Logger Config

The logger config in `config.toml` is listed below with default values.

```toml
[logger]
filter = "info"
log_to_console = true
console_show_file_and_line = false
log_path = "logs/"
log_to_file = true
metrics = true
```

`filter` is the root logger filter, must be one of `off`, `trace`, `debug`, `info`, `warn` and `error`.

If `log_to_console` is `true`, logs like below will be logged to console.

```
[2019-12-02T10:02:45.779337+08:00 INFO overlord::state::process] Overlord: state receive commit event height 11220, round 0
```

If `console_show_file_and_line` is `true`, log file and line number will also be logged to console, pretty useful for debugging.

```
[2019-12-02T10:05:28.343228+08:00 INFO core_network::peer_manager core/network/src/peer_manager/mod.rs:1035] network: PeerId(QmYSZUy3G5Mf5GSTKfH7LXJeFJrVW59rX1qPPfapuH7AUw): connected peer_ip(s): []
```

If `log_to_file` is true, logs like below will be logged to `{log_path}/muta.log`.
It is json format, good for machine understanding.

```
{"time":"2019-12-01T22:01:57.839042+08:00","message":"network: PeerId(QmYSZUy3G5Mf5GSTKfH7LXJeFJrVW59rX1qPPfapuH7AUw): connect addrs [\"/ip4/0.0.0.0/tcp/1888\"]","module_path":"core_network::peer_manager","file":"core/network/src/peer_manager/mod.rs","line":591,"level":"INFO","target":"core_network::peer_manager","thread":"tokio-runtime-worker-0","thread_id":123145432756224,"mdc":{}}
```

This crate uses `log4rs` to init the logger, but you don't need to add dependency for that. After invoking the `init` function in this crate, you can use `log` crate to log.

## Metrics

Metrics is an independent logger, it `metrics` is `true`, the metrics will be logged to `{log_path}/metrics.log`.

```
{"time":"2019-12-01T22:02:49.035084+08:00","message":"{\"height\":7943,\"name\":\"save_block\",\"ordered_tx_num\":0}","module_path":"common_logger","file":"common/logger/src/lib.rs","line":83,"level":"TRACE","target":"metrics","thread":"tokio-runtime-worker-3","thread_id":123145445486592,"mdc":{}}
```

If you want to use log metrics in a module, you need to add this crate as dependency and use the code below to add a metric. The `name` field is reserved, please avoid using this as a key in your metrics.

```rust
common_logger::metrics("save_block", common_logger::object! {
    "height" => block.header.height,
    "ordered_tx_num" => block.ordered_tx_hashes.len(),
});
```

This signature of the function is showed below. The `JsonValue` is a `enum` from [`json crate`](https://docs.rs/json/0.12.0/json/enum.JsonValue.html).

```rust
pub fn metrics(name: &str, mut content: JsonValue)
```

## Structured Event Log With TraceId Included

Structured event log api provide a convenient way to log structured json data. It's signature is provided as below:

```rust
pub fn log(level: Level, module: &str, event: &str, ctx: &Context, mut msg: JsonValue)
```

`module` should be your component name, `event` is just event name, better begin with 4 chars with 4 digits
to identify this event. `Context` is used to extract trace id. `msg` is `JsonValue` which is same as `metrics`.

Useage example:

```rust
common_logger::log(Level::Info, "network", "netw0001", &ctx, common_logger::json!({"music", "beautiful world"; "movie", "fury"}));
```

## Yaml File

The `log.yml` in this crate is the yaml style config of log4rs with default logger config.

If you need more customized configurations, you can copy the file to some config path, edit the file, and replace the `init` function with `log4rs::init_file("/path/to/log.yml", Default::default()).unwrap();`.
