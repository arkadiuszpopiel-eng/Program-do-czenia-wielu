//! Jawne limity ręcznej diagnozyCPU; konfiguracja produkcyjna nie jest zmieniana.
pub fn configure(config: &mut providers_local_impl::LocalConfig) -> (Option<u64>, u64, bool) {
    let seconds = std::env::var("ALFA_CPU_DIAGNOSTIC_SECONDS")
        .ok()
        .map(|v| v.parse::<u64>().expect("sekundy diagnozyCPU"));
    if let Some(s) = seconds {
        assert!((1..=1800).contains(&s));
        config.startup_timeout = std::time::Duration::from_secs(s);
        config.timeouts.first_token = std::time::Duration::from_secs(s);
        config.timeouts.idle = std::time::Duration::from_secs(s);
    }
    let empty =
        seconds.is_some() && std::env::var("ALFA_CPU_DIAGNOSTIC_EMPTY").as_deref() == Ok("1");
    (seconds, seconds.map_or(120, |s| s * 3 + 120), empty)
}
