//! REST API requests over the Spot network (feature `spot`).
//!
//! Port of the Go `rest` package's `spot.go`. Instead of an HTTP request to
//! `www.atonline.com/_special/rest/<path>`, a request is framed as
//! `{"path": <path>, "verb": <method>, "params": <param>}` and sent over an
//! authenticated [Spot](https://github.com/KarpelesLab/spotlib) connection to
//! the platform API endpoint [`P_API_TARGET`] (`@/p_api`); the reply is the same
//! REST [`Response`] envelope (`result`/`data`/`error`/`code`).
//!
//! The Spot connection's cryptographic identity **is** the authentication
//! context — no `Sec-ClientId` header, OAuth token, or API-key signature is sent
//! (unlike the HTTP path). This is what lets a browser (wasm32) reach the
//! backend with neither CORS nor a client id, riding the same transport the
//! spotlib client already uses.
//!
//! To keep this crate free of a `spotlib` dependency (and avoid a dependency
//! loop), the transport is abstracted behind the [`SpotClient`] trait — mirror
//! of the Go `SpotClient` interface. The caller implements it over its own
//! spotlib client (typically a one-line adapter around `Client::query`).
//!
//! ```no_run
//! # #[cfg(feature = "spot")] {
//! use klbfw::spot::{SpotClient, spot_apply};
//! # struct MyClient;
//! impl SpotClient for MyClient {
//!     async fn query(&self, target: &str, body: &[u8]) -> Result<Vec<u8>, String> {
//!         // self.spot.query(target, body, timeout).await ...
//!         # let _ = (target, body); Ok(Vec::new())
//!     }
//! }
//! # async fn ex(c: &MyClient) -> Result<(), klbfw::RestError> {
//! let keys: serde_json::Value = spot_apply(c, "Crypto/WalletSign:keys", "GET", ()).await?;
//! # let _ = keys; Ok(()) }
//! # }
//! ```

use serde::Serialize;
use serde_json::json;

use crate::error::{RestError, Result};
use crate::response::Response;

/// The platform REST-over-Spot endpoint queried by [`spot_do_request`] — the
/// target address the Go `SpotDo` sends to.
pub const P_API_TARGET: &str = "@/p_api";

/// A transport that carries a request over the Spot network and returns the
/// reply bytes. Implemented by the caller over its `spotlib::Client` (a trait,
/// not a direct dependency, to avoid a dependency loop — mirrors the Go
/// `SpotClient` interface).
///
/// `async fn` in trait: this crate targets Rust ≥ 1.75. The trait is not meant
/// to be used as `dyn SpotClient`; the REST helpers take `impl SpotClient`.
#[allow(async_fn_in_trait)]
pub trait SpotClient {
    /// Send `body` to `target` (e.g. [`P_API_TARGET`]) over the Spot connection
    /// and return the reply bytes. On failure, return a human-readable message
    /// (surfaced as [`RestError::Spot`]).
    async fn query(&self, target: &str, body: &[u8]) -> std::result::Result<Vec<u8>, String>;
}

/// Execute a REST API request over Spot and return the raw [`Response`] (port of
/// Go `SpotDo`): frames `{path, verb, params}`, queries [`P_API_TARGET`], and
/// parses the envelope. Does not inspect `result` — use [`spot_apply`] to fail
/// on an error envelope, or check [`Response::result`] yourself.
pub async fn spot_do_request<C, P>(
    client: &C,
    path: &str,
    method: &str,
    param: P,
) -> Result<Response>
where
    C: SpotClient,
    P: Serialize,
{
    let req = json!({ "path": path, "verb": method, "params": param });
    let buf = serde_json::to_vec(&req)?;
    let respbuf = client
        .query(P_API_TARGET, &buf)
        .await
        .map_err(RestError::Spot)?;
    let resp: Response = serde_json::from_slice(&respbuf)?;
    Ok(resp)
}

/// Execute a REST API request over Spot and deserialize the response `data` into
/// `T` (port of Go `SpotApply`/`SpotAs`). Unlike the Go original this fails with
/// [`RestError::from_response`] when the envelope's `result` is not `"success"`,
/// so API errors surface as `Err` rather than a confusing deserialize failure.
pub async fn spot_apply<C, T, P>(client: &C, path: &str, method: &str, param: P) -> Result<T>
where
    C: SpotClient,
    T: serde::de::DeserializeOwned,
    P: Serialize,
{
    let resp = spot_do_request(client, path, method, param).await?;
    if resp.result != "success" {
        return Err(RestError::from_response(resp));
    }
    resp.apply()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An in-memory `SpotClient` that returns a canned reply, to exercise the
    /// framing + envelope parsing without a real Spot connection.
    struct MockSpot {
        reply: Vec<u8>,
    }
    impl SpotClient for MockSpot {
        async fn query(&self, target: &str, body: &[u8]) -> std::result::Result<Vec<u8>, String> {
            assert_eq!(target, P_API_TARGET);
            // The request must be the {path,verb,params} envelope.
            let v: serde_json::Value = serde_json::from_slice(body).unwrap();
            assert_eq!(v["path"], "Test:endpoint");
            assert_eq!(v["verb"], "POST");
            assert_eq!(v["params"]["x"], 1);
            Ok(self.reply.clone())
        }
    }

    // A tiny synchronous block-on so the async helpers can be tested without a
    // runtime dependency (the futures here never yield — the mock is immediate).
    fn block_on<F: std::future::Future>(mut f: F) -> F::Output {
        use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
        fn noop(_: *const ()) {}
        fn clone(_: *const ()) -> RawWaker {
            RawWaker::new(std::ptr::null(), &VT)
        }
        static VT: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
        let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VT)) };
        let mut cx = Context::from_waker(&waker);
        // SAFETY: f is not moved after pinning (owned local, shadowed).
        let mut f = unsafe { std::pin::Pin::new_unchecked(&mut f) };
        loop {
            if let Poll::Ready(v) = f.as_mut().poll(&mut cx) {
                return v;
            }
        }
    }

    #[test]
    fn spot_do_request_frames_and_parses() {
        let client = MockSpot {
            reply: br#"{"result":"success","data":{"ok":true}}"#.to_vec(),
        };
        let resp = block_on(spot_do_request(
            &client,
            "Test:endpoint",
            "POST",
            json!({ "x": 1 }),
        ))
        .unwrap();
        assert_eq!(resp.result, "success");
        assert_eq!(resp.data.unwrap()["ok"], true);
    }

    #[test]
    fn spot_apply_deserializes_data() {
        #[derive(serde::Deserialize)]
        struct Out {
            ok: bool,
        }
        let client = MockSpot {
            reply: br#"{"result":"success","data":{"ok":true}}"#.to_vec(),
        };
        let out: Out = block_on(spot_apply(
            &client,
            "Test:endpoint",
            "POST",
            json!({ "x": 1 }),
        ))
        .unwrap();
        assert!(out.ok);
    }

    #[test]
    fn spot_apply_surfaces_api_error() {
        let client = MockSpot {
            reply: br#"{"result":"error","error":"nope","code":403}"#.to_vec(),
        };
        let err = block_on(spot_apply::<_, serde_json::Value, _>(
            &client,
            "Test:endpoint",
            "POST",
            json!({ "x": 1 }),
        ))
        .unwrap_err();
        assert!(err.is_permission_denied());
    }
}
