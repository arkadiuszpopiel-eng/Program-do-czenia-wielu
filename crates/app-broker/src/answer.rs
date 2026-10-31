//! Odpowiedzi Sędziego Cieni przez łącze IPC → wyniki traitu `Broker` (fail-closed: błąd łącza
//! = Broker niedostępny, nieoczekiwana odpowiedź = błąd).

use safety_broker_contract::BrokerError;
use safety_broker_contract::ipc::Response;

use crate::link::LinkError;

pub(crate) fn unavailable(e: &LinkError) -> BrokerError {
    BrokerError::AuditUnavailable(format!("Sędzia Cieni niedostępny — {e}"))
}

fn unexpected() -> BrokerError {
    BrokerError::InvalidRequest("nieoczekiwana odpowiedź Sędziego Cieni".into())
}

/// Odpowiedź → wynik: błąd łącza = Broker niedostępny (fail-closed), `Error` = błąd Brokera.
pub(crate) fn answer<T>(
    r: Result<Response, LinkError>,
    pick: impl FnOnce(Response) -> Option<T>,
) -> Result<T, BrokerError> {
    match r {
        Err(e) => Err(unavailable(&e)),
        Ok(Response::Error(e)) => Err(e),
        Ok(other) => pick(other).ok_or_else(unexpected),
    }
}

pub(crate) fn count(r: Response) -> Option<usize> {
    match r {
        Response::Count(n) => Some(usize::try_from(n).unwrap_or(usize::MAX)),
        _ => None,
    }
}

pub(crate) fn ok(r: Response) -> Option<()> {
    matches!(r, Response::Ok).then_some(())
}
