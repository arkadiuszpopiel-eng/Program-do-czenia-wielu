// Stan Brokera (komenda `broker_status`, zdarzenie `BrokerStatus`) — kształt 1:1
// z `crates/app-api/src/dto/broker.rs` (pola snake_case). UI tylko pokazuje stan i wyjaśnienie;
// zatwierdzanie odbywa się wyłącznie w oknie Brokera (PLAN §8.2), nigdy w WebView.
export type { BrokerLinkState, BrokerMode, BrokerStatusView } from './generated/ipc';
