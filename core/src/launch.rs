//! What must be settled in the process environment before anything else runs.
//!
//! [`prepare_environment`] is the first thing `main()` calls, while the process still has one
//! thread: mutating the environment is unsound once any other thread exists, and GStreamer
//! and libsoup read these variables back only once. It is the one place in the backend that
//! may call `env::set_var` or `env::remove_var` (tests/source_guards.rs).

/// Call first in `main()`, before any thread is started.
pub fn prepare_environment() {
    // Read once, used by both the bypass record and the scrub below, so
    // the two reason about the same sidecar structurally rather than
    // because two reads microseconds apart happened to agree.
    let sidecar = crate::config_dir_for_env()
        .and_then(|d| crate::proxy::read_sidecar(&d));

    // Record whether an ambient bypass list will still be in the
    // environment once this function is done — read here, at the only
    // moment the answer is knowable, because nothing may mutate the
    // environment once other threads exist.
    //
    // Outside the `if let` below on purpose: with neither `$XDG_CONFIG_HOME`
    // nor `$HOME` set there is no sidecar to read, no scrub, and therefore
    // an ambient bypass list that survives. Recording inside would skip
    // the call and read back `false` — fail-open, the direction this is
    // here to close.
    {
        let present = crate::proxy::SCRUBBED_PROXY_ENV_VARS
            .into_iter()
            .any(|v| std::env::var_os(v).is_some_and(|s| !s.is_empty()));
        crate::proxy::remember_launch_bypass(should_record_launch_bypass(
            present,
            sidecar.clone(),
        ));
    }

    // Own the proxy environment before anything can read it — but only
    // while SONE is actually proxying. `curlhttpsrc` reads `no_proxy` at
    // element construction and honours it over an explicitly-set proxy
    // property, with no property to override it, so an ambient value has
    // to be gone before any element exists; clearing it later is too late,
    // and `set_var` is unsound once other threads exist.
    //
    // Conditional, and that is the point. `Direct` means the system's own
    // configuration applies: a user behind a corporate proxy with
    // `http_proxy` exported works today, and scrubbing unconditionally
    // would silently route them direct — the exact silent-degradation
    // regression this whole design exists to prevent.
    //
    // The settings file is encrypted and its key needs a keyring and an
    // AppHandle, neither of which exists this early, so the decision comes
    // from the plaintext sidecar, which carries the enabled flag and the
    // proxy type and nothing else.
    if should_scrub_proxy_env(sidecar) {
        // Capture before removing, never after. `Direct` means the
        // system's own configuration applies, and if the user turns
        // SONE's proxy off later in this session that configuration is
        // the only thing routing them — but it lived in exactly the
        // variables about to be deleted. reqwest reads them back from
        // this capture instead of from an environment we emptied.
        //
        // The capture is the full list and the removal is only the
        // bypass pair. That is not an oversight: this is the sole
        // sound moment to read these values, and stage 4a needs the
        // per-scheme ones when it starts scrubbing them.
        let captured: Vec<(String, String)> = crate::proxy::PROXY_ENV_VARS
            .into_iter()
            .filter_map(|v| std::env::var(v).ok().map(|value| (v.to_string(), value)))
            .collect();
        crate::proxy::remember_scrubbed_env(captured);

        for v in crate::proxy::SCRUBBED_PROXY_ENV_VARS {
            std::env::remove_var(v);
        }
    }

    // libsoup3 over HTTP/2 can hang forever on a connection the CDN dropped
    // while the buffer was full, starving playback with no error to retry on
    // (libsoup#296, sone#252). HTTP/1.1 surfaces the drop and souphttpsrc
    // resumes with a Range request. libsoup reads this once per process and
    // only checks presence, so any preset value already forces HTTP/1.1.
    if std::env::var_os("SOUP_FORCE_HTTP1").is_none() {
        std::env::set_var("SOUP_FORCE_HTTP1", "1");
    }
}

/// Whether the startup scrub runs, given whatever the launch sidecar said.
///
/// Pure so the one rule that matters can be asserted without touching the
/// environment: anything other than a recorded, enabled proxy leaves the
/// variables exactly as the user's shell set them. A missing sidecar (first
/// launch, or a write that failed) is *not* a default — it is "not proxying".
fn should_scrub_proxy_env(sidecar: Option<(bool, String)>) -> bool {
    matches!(sidecar, Some((true, _)))
}

/// Whether the launch-time bypass list is recorded as still in force, given
/// whether one was present in the environment and whatever the launch sidecar
/// said.
///
/// Pure for the same reason as `should_scrub_proxy_env`, and needed more: the
/// gate is load-bearing in *both* directions and is one token from either
/// failure. Dropping the scrub term refuses every proxied tier, for the whole
/// session, for a user who launched with the proxy on and `no_proxy` exported —
/// a launch that scrubbed has no ambient bypass list left to defeat anything.
/// Dropping the whole call reopens the containment hole: `curlhttpsrc` reads
/// `no_proxy` when the element is constructed and forwards it as
/// `CURLOPT_NOPROXY`, which beats the `proxy` property, so a bypass list that
/// survived startup silently sends audio out on the user's real address.
fn should_record_launch_bypass(present: bool, sidecar: Option<(bool, String)>) -> bool {
    present && !should_scrub_proxy_env(sidecar)
}

/// System GStreamer plugin directories, probed in order, and only when the
/// process is not running from a bundle.
#[cfg(test)]
mod tests {
    use super::{should_record_launch_bypass, should_scrub_proxy_env};

    #[test]
    fn a_recorded_enabled_proxy_scrubs_whatever_its_type() {
        assert!(should_scrub_proxy_env(Some((true, "http".into()))));
        assert!(should_scrub_proxy_env(Some((true, "socks5".into()))));
    }

    /// The regression this task must not introduce: the proxy toggle off means
    /// the system's own configuration applies, so an exported `http_proxy` has
    /// to survive startup untouched.
    #[test]
    fn the_proxy_toggle_off_leaves_the_environment_alone() {
        assert!(!should_scrub_proxy_env(Some((false, "http".into()))));
        assert!(!should_scrub_proxy_env(Some((false, "socks5".into()))));
    }

    /// No sidecar at all — every launch before this feature shipped, and every
    /// first launch after it.
    #[test]
    fn an_unknown_mode_leaves_the_environment_alone() {
        assert!(!should_scrub_proxy_env(None));
    }

    /// The whole truth table, because each row fails a different way and the
    /// two `false` rows fail in opposite directions.
    ///
    /// Row `(present, proxying)`: the launch scrubbed the bypass list, so
    /// nothing is left to defeat the proxy. Recording `true` here refuses
    /// every proxied tier for a session that is perfectly fine, and restarting
    /// reproduces it exactly — the failure that pins the `!should_scrub` term.
    #[test]
    fn a_launch_that_scrubbed_has_no_surviving_bypass_list() {
        assert!(!should_record_launch_bypass(
            true,
            Some((true, "http".into()))
        ));
        assert!(!should_record_launch_bypass(
            true,
            Some((true, "socks5".into()))
        ));
    }

    /// Row `(present, not proxying)`: nothing scrubbed, so the bypass list is
    /// still in the environment and will beat the `proxy` property the moment
    /// the user turns the proxy on. This is the containment hole, and the row
    /// that pins the call existing at all.
    #[test]
    fn a_bypass_list_that_survived_an_unscrubbed_launch_is_recorded() {
        assert!(should_record_launch_bypass(true, None));
        assert!(should_record_launch_bypass(
            true,
            Some((false, "http".into()))
        ));
    }

    /// Row `(absent, _)`: no bypass list at launch, nothing to record, whatever
    /// the sidecar said. Recording `true` on either of these refuses proxied
    /// playback for every user who never exported `no_proxy` at all.
    #[test]
    fn no_bypass_list_at_launch_records_nothing_either_way() {
        assert!(!should_record_launch_bypass(false, None));
        assert!(!should_record_launch_bypass(
            false,
            Some((false, "http".into()))
        ));
        assert!(!should_record_launch_bypass(
            false,
            Some((true, "http".into()))
        ));
    }
}
