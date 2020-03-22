Tiny crate providing added functionality to the futures-rs threadpool executor.
The `futures::future::ThreadPool` executor currently has no way built in to handle panics, manually stop the pool execution or bubble up errors back to the caller.
The crate works around these limitations (it might get obsolete once [this open issue will be resolved](https://github.com/rust-lang/futures-rs/issues/1468)).

Use cases for the crate are:
* Stop executing futures in case any of future that runs on it faces an unrecoverable error and returns an Err().
* Let the caller handle the error.
* Stop the threadpool and its spawned tasks on user request.

# Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
poolparty = "0.1"
```

The following example demonstrates the handling of a failing task:
```rust
async fn forever() -> Result<(),String> {
    loop {}
}

async fn fail(msg: String) -> Result<(),String> {
    Err(msg)
}

let mut pool = StoppableThreadPool::new()?;
let err = "fail_function_called".to_string();
pool.spawn(fail(err.clone()));
pool.spawn(forever());

assert_eq!(
    pool.observe().await.unwrap_err(),
    err
)
```

Have a look at the tests in [lib.rs](https://github.com/xermicus/poolparty/blob/master/src/lib.rs#L114) for more usage examples.

# License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in poolparty by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
