mod logging;
mod signal;
mod socks;
use clap::clap_app;
use futures::future::{self, Either};
use tokio::{self, runtime::Builder};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const ADDRESS: &str = "0.0.0.0:8080";

fn main() {
    let app = clap_app!(socks5_rust =>
        (version:VERSION)
        (about: "A fast socks5 debug server.")
        (@arg ADDRESS: -a --address default_value(ADDRESS) "socks5 server listen address")
    );

    logging::init();
    let m = app.get_matches();

    let addr = match m.value_of("ADDRESS") {
        Some(addr) => addr,
        None => ADDRESS,
    };

    let mut builder = Builder::new_current_thread();

    let runtime = builder.enable_all().build().expect("create tokio Runtime");
    runtime.block_on(async move {
        let abort_signal = signal::create_signal_monitor();
        let server = socks::server(addr);

        tokio::pin!(abort_signal);
        tokio::pin!(server);

        match future::select(server, abort_signal).await {
            // Server future resolved without an error. This should never happen.
            Either::Left((Ok(..), ..)) => panic!("server exited unexpectly"),
            // Server future resolved with error, which are listener errors in most cases
            Either::Left((Err(err), ..)) => panic!("aborted with {}", err),
            // The abort signal future resolved. Means we should just exit.
            Either::Right(_) => (),
        }
    });
}
