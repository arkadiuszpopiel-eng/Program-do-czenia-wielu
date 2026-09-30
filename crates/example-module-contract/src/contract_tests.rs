//! Współdzielony test kontraktowy (feature `contract-tests`).
//! Ten sam zestaw przypadków uruchamia `example-module-impl` i `example-module-fake`.

use crate::{Echo, EchoError, MAX_INPUT_CHARS};

/// Echo zwraca wejście bez zmian i liczy znaki (nie bajty).
pub async fn echo_returns_input<E: Echo>(echo: &E) {
    let reply = echo
        .echo("zażółć gęślą")
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(reply.text, "zażółć gęślą");
    assert_eq!(reply.chars, 12);
}

/// Numer kolejny rośnie o 1 przy każdym udanym wywołaniu.
pub async fn seq_is_monotonic<E: Echo>(echo: &E) {
    let a = echo.echo("a").await.unwrap_or_else(|e| panic!("{e}"));
    let b = echo.echo("b").await.unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(b.seq, a.seq + 1);
}

/// Puste i za długie wejście są odrzucane bez zwiększania numeru kolejnego.
pub async fn invalid_input_rejected<E: Echo>(echo: &E) {
    let before = echo.echo("x").await.unwrap_or_else(|e| panic!("{e}")).seq;
    assert_eq!(echo.echo("").await, Err(EchoError::Empty));
    let long = "y".repeat(MAX_INPUT_CHARS + 1);
    assert!(matches!(
        echo.echo(&long).await,
        Err(EchoError::TooLong { .. })
    ));
    let after = echo.echo("x").await.unwrap_or_else(|e| panic!("{e}")).seq;
    assert_eq!(after, before + 1);
}

/// Uruchamia cały zestaw; `factory` (async) daje świeżą, uruchomioną instancję.
pub async fn run_all<E, F, Fut>(factory: F)
where
    E: Echo,
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = E>,
{
    echo_returns_input(&factory().await).await;
    seq_is_monotonic(&factory().await).await;
    invalid_input_rejected(&factory().await).await;
}
